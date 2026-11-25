//! Local AI ghost completions.
//!
//! Kodemo talks to a local llama.cpp server through its `/completion` endpoint
//! using Qwen's fill-in-the-middle tokens. Ollama (`/api/generate`) and plain
//! OpenAI-compatible completion endpoints are accepted too, so existing user
//! settings keep working. No network service is required beyond localhost.

use crate::settings::{home_dir, Settings};
use crate::syntax::Language;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};
use std::{fs, thread};

const FIM_PREFIX: &str = "<|fim_prefix|>";
const FIM_SUFFIX: &str = "<|fim_suffix|>";
const FIM_MIDDLE: &str = "<|fim_middle|>";
const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:8090/completion";
// Ghost suggestions only need local context. Keeping the prompt small matters
// a lot on low-end CPUs: prefill is the dominant cost of a request. Once the
// user accepts a suggestion, the request widens to offer a full block.
const MAX_PREFIX_CHARS: usize = 160;
const MAX_SUFFIX_CHARS: usize = 64;
/// Three previous lines give the model enough structure to infer indentation
/// and naming. The anchor sits on a line boundary, so llama.cpp's prompt cache
/// makes this extra context almost free after the first request.
const CONTEXT_LINES: usize = 3;
/// Accepted suggestions make the next request grow by this many lines and
/// tokens, up to `MAX_BOOST` steps.
const MAX_BOOST: u8 = 3;

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub text: String,
    /// Character index in the document where this suggestion starts.
    pub start: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AiStatus {
    Off,
    StartingServer,
    Ready,
    Thinking,
    Unavailable(String),
}

impl AiStatus {
    pub fn is_busy(&self) -> bool {
        matches!(self, Self::StartingServer | Self::Thinking)
    }

    pub fn short(&self) -> &'static str {
        match self {
            Self::Off => "AI off",
            Self::StartingServer => "Starting Qwen…",
            Self::Ready => "AI ready",
            Self::Thinking => "Qwen thinking…",
            Self::Unavailable(_) => "AI unavailable",
        }
    }
}

enum AiEvent {
    ServerReady,
    ServerFailed(String),
    Completion { seq: u64, result: Result<String, String> },
}

pub struct AiEngine {
    pub status: AiStatus,
    pub suggestion: Option<Suggestion>,
    pub last_error: Option<String>,
    pub server_log: PathBuf,
    enabled: bool,
    endpoint: String,
    model: String,
    max_tokens: usize,
    auto_start: bool,
    server: Option<Child>,
    tx: Sender<AiEvent>,
    rx: Receiver<AiEvent>,
    next_seq: u64,
    pending_seq: Option<u64>,
    request_start: usize,
    /// The text before the caret on its line when the request was sent.
    request_line: String,
    request_started: Option<Instant>,
    starting_since: Option<Instant>,
    last_latency: Option<Duration>,
    request_language: Option<Language>,
    last_activity: Instant,
    /// How many suggestions in a row the user accepted. Each step asks the
    /// model for a longer block, so accepting one line invites the next few.
    boost: u8,
}

impl Drop for AiEngine {
    fn drop(&mut self) {
        self.stop_server();
    }
}

impl AiEngine {
    pub fn new(settings: &Settings) -> Self {
        let (tx, rx) = channel();
        let mut engine = Self {
            status: AiStatus::Off,
            suggestion: None,
            last_error: None,
            server_log: cache_path(),
            enabled: settings.ai_enabled,
            endpoint: settings.ai_endpoint.clone(),
            model: settings.ai_model.clone(),
            max_tokens: settings.ai_max_tokens,
            auto_start: settings.ai_auto_start,
            server: None,
            tx,
            rx,
            next_seq: 1,
            pending_seq: None,
            request_start: 0,
            request_line: String::new(),
            request_started: None,
            starting_since: None,
            last_latency: None,
            request_language: None,
            last_activity: Instant::now(),
            boost: 0,
        };
        engine.configure(settings);
        engine
    }

    pub fn configure(&mut self, settings: &Settings) {
        let changed = self.endpoint != settings.ai_endpoint
            || self.model != settings.ai_model
            || self.auto_start != settings.ai_auto_start;
        self.enabled = settings.ai_enabled;
        self.endpoint = settings.ai_endpoint.trim().to_owned();
        self.model = settings.ai_model.trim().to_owned();
        self.max_tokens = settings.ai_max_tokens.clamp(8, 256);
        self.auto_start = settings.ai_auto_start;
        self.suggestion = None;
        self.pending_seq = None;
        self.request_started = None;
        self.starting_since = None;
        self.last_error = None;
        if changed {
            self.stop_server();
        }
        if !self.enabled {
            self.stop_server();
            self.status = AiStatus::Off;
            return;
        }
        if self.endpoint.is_empty() {
            self.status = AiStatus::Unavailable("No AI endpoint is configured.".to_owned());
            return;
        }
        let Ok(target) = Endpoint::parse(&self.endpoint) else {
            self.status = AiStatus::Unavailable("The AI endpoint URL is invalid.".to_owned());
            return;
        };
        if !target.is_local() {
            // Assume a remote or LAN server is managed elsewhere.
            self.status = AiStatus::Ready;
            return;
        }
        if target.reachable() {
            // A llama.cpp server accepts TCP connections before the model has
            // finished loading. Keep the status busy until /health is green.
            if target.port == 8090 && !health_ok(&target.host, target.port) {
                self.status = AiStatus::StartingServer;
                self.starting_since.get_or_insert_with(Instant::now);
                self.spawn_server_waiter(target.port);
                return;
            }
            self.status = AiStatus::Ready;
            self.starting_since = None;
            return;
        }
        // The saved endpoint may be an Ollama URL that is no longer running.
        // When Kodemo's bundled llama.cpp server and Qwen model exist, fall
        // back to them instead of leaving ghost completions dead.
        let mut target = target;
        if target.port != 8090
            && settings.find_llama_server().is_some()
            && settings.find_model().is_some()
        {
            if let Ok(fallback) = Endpoint::parse(DEFAULT_ENDPOINT) {
                if fallback.reachable() {
                    self.endpoint = DEFAULT_ENDPOINT.to_owned();
                    self.status = AiStatus::Ready;
                    self.starting_since = None;
                    return;
                }
                target = fallback;
                self.endpoint = DEFAULT_ENDPOINT.to_owned();
            }
        }
        if !self.auto_start {
            self.status = AiStatus::Unavailable(format!(
                "Nothing is listening on 127.0.0.1:{}. Start llama-server or enable auto-start.",
                target.port
            ));
            return;
        }
        if let Some(server) = self.server.as_mut() {
            if matches!(server.try_wait(), Ok(None)) {
                self.status = AiStatus::StartingServer;
                self.starting_since.get_or_insert_with(Instant::now);
                self.spawn_server_waiter(target.port);
                return;
            }
            self.server = None;
        }
        let Some(server_path) = settings.find_llama_server() else {
            self.status = AiStatus::Unavailable(
                "llama-server was not found. Set its path in Settings, or install it under ~/.local/bin."
                    .to_owned(),
            );
            return;
        };
        let Some(model_path) = settings.find_model() else {
            self.status = AiStatus::Unavailable(
                "No Qwen GGUF model was found. Put it in ~/.local/share/kodemo/models/ or choose it in Settings."
                    .to_owned(),
            );
            return;
        };
        match self.spawn_server(&server_path, &model_path, &target) {
            Ok(()) => {
                self.endpoint = target.url();
                self.status = AiStatus::StartingServer;
                self.starting_since = Some(Instant::now());
                self.spawn_server_waiter(target.port);
            }
            Err(err) => {
                self.status = AiStatus::Unavailable(err.clone());
                self.last_error = Some(err);
            }
        }
    }

    pub fn restart(&mut self, settings: &Settings) {
        self.stop_server();
        self.status = AiStatus::Off;
        self.configure(settings);
    }

    /// The endpoint actually in use, which can differ from the saved one when
    /// Kodemo falls back to its bundled llama.cpp server.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn detected_language(&self) -> Option<Language> {
        self.request_language
    }

    /// Time spent in the current busy phase, used by the progress UI.
    pub fn elapsed(&self) -> Option<Duration> {
        match self.status {
            AiStatus::Thinking => self.request_started.map(|start| start.elapsed()),
            AiStatus::StartingServer => self.starting_since.map(|start| start.elapsed()),
            _ => None,
        }
    }

    /// How long the last finished request took, including prompt prefill.
    pub fn last_latency(&self) -> Option<Duration> {
        self.last_latency
    }

    /// Stop the local server entirely (Energy Saver). `configure` or
    /// `restart` brings it back.
    pub fn suspend(&mut self) {
        self.stop_server();
        self.suggestion = None;
        self.pending_seq = None;
        self.request_started = None;
        self.starting_since = None;
        self.boost = 0;
        self.status = AiStatus::Off;
    }

    pub fn invalidate(&mut self) {
        self.suggestion = None;
        self.pending_seq = None;
        self.request_started = None;
        if self.status == AiStatus::Thinking {
            self.status = AiStatus::Ready;
        }
    }

    /// The user accepted a suggestion: ask for a longer block next time.
    pub fn boost(&mut self) {
        self.boost = (self.boost + 1).min(MAX_BOOST);
    }

    /// Plain typing means the user wants a fresh, short hint again.
    pub fn reset_boost(&mut self) {
        self.boost = 0;
    }

    fn context_lines(&self) -> usize {
        CONTEXT_LINES + self.boost as usize * 2
    }

    fn prefix_budget(&self) -> usize {
        (MAX_PREFIX_CHARS + self.boost as usize * 160).min(640)
    }

    pub fn tick(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(AiEvent::ServerReady) => {
                    self.starting_since = None;
                    if !matches!(self.status, AiStatus::Thinking) {
                        self.status = AiStatus::Ready;
                    }
                    self.last_error = None;
                    self.last_activity = Instant::now();
                }
                Ok(AiEvent::ServerFailed(err)) => {
                    self.starting_since = None;
                    self.status = AiStatus::Unavailable(err.clone());
                    self.last_error = Some(err);
                    self.last_activity = Instant::now();
                }
                Ok(AiEvent::Completion { seq, result }) => {
                    if self.pending_seq == Some(seq) {
                        self.pending_seq = None;
                        self.last_latency = self.request_started.take().map(|t| t.elapsed());
                        self.last_activity = Instant::now();
                        match result {
                            Ok(raw) => {
                                self.status = AiStatus::Ready;
                                self.last_error = None;
                                let max_lines = 1 + self.boost as usize * 4;
                                if let Some(text) = sanitize(&raw, max_lines) {
                                    // A small model sometimes echoes the text
                                    // that is already on the line; that is not
                                    // a completion and would duplicate code.
                                    let echoes_line = !self.request_line.is_empty()
                                        && text
                                            .lines()
                                            .next()
                                            .is_some_and(|line| {
                                                line.trim_end() == self.request_line.trim_end()
                                            });
                                    if !echoes_line {
                                        self.suggestion = Some(Suggestion {
                                            text,
                                            start: self.request_start,
                                        });
                                    }
                                }
                            }
                            Err(err) => {
                                self.last_error = Some(err.clone());
                                self.status = AiStatus::Unavailable(err);
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
    }

    /// Ask for a completion after the given character offset.
    ///
    /// The active language is detected by the IDE and passed to Qwen so the
    /// suggestion matches the file being edited; it is also used to shape the
    /// FIM prompt with a language-specific comment header.
    pub fn request(
        &mut self,
        text: &str,
        cursor_char: usize,
        language: Language,
        settings: &Settings,
    ) {
        if !self.enabled || self.pending_seq.is_some() {
            return;
        }
        let before_cursor: String = text.chars().take(cursor_char).collect();
        let line_prefix: String = before_cursor
            .rsplit('\n')
            .next()
            .unwrap_or_default()
            .to_owned();
        let before_line = before_cursor
            .rsplit_once('\n')
            .map(|(before, _)| before)
            .unwrap_or("");
        let at_line_start = line_prefix.trim().is_empty();
        // A fresh line is a great place for a full-line suggestion, but only
        // when there is already some code above the cursor to learn from.
        let fresh_line = at_line_start && !before_line.trim().is_empty();
        if line_prefix.trim().chars().count() < settings.completion_min_chars
            && !fresh_line
        {
            return;
        }
        let chars: Vec<char> = text.chars().collect();
        let cursor_char = cursor_char.min(chars.len());
        self.request_line = line_prefix.trim_end().to_owned();
        let start = prefix_start(&chars, cursor_char, self.context_lines(), self.prefix_budget());
        let suffix_budget = MAX_SUFFIX_CHARS + self.boost as usize * 120;
        let end = (cursor_char + suffix_budget).min(chars.len());
        // Starting at a stable line boundary lets llama.cpp reuse its prompt
        // cache while the user types on the same line.
        let prefix: String = chars[start..cursor_char].iter().collect();
        let suffix: String = chars[cursor_char..end].iter().collect();
        let preamble = language_preamble(language).unwrap_or_default();
        let prompt = format!("{FIM_PREFIX}{preamble}{prefix}{FIM_SUFFIX}{suffix}{FIM_MIDDLE}");
        let endpoint = self.endpoint.clone();
        let model = self.model.clone();
        // One line of ghost text never needs dozens of tokens, and stopping
        // early keeps the editor responsive on slow machines. After an
        // accepted suggestion the budget grows so a whole block can arrive.
        let max_tokens = if self.boost == 0 {
            self.max_tokens.clamp(12, 20)
        } else {
            (self.max_tokens.max(64) + self.boost as usize * 32).min(240)
        };
        let multi_line = self.boost > 0;
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1).max(1);
        self.pending_seq = Some(seq);
        self.request_start = cursor_char;
        self.request_started = Some(Instant::now());
        self.starting_since = None;
        self.request_language = Some(language);
        self.status = AiStatus::Thinking;
        let tx = self.tx.clone();
        let _ = thread::Builder::new()
            .name("kodemo-ai-request".to_owned())
            .spawn(move || {
                let result =
                    completion_request(&endpoint, &model, &prompt, max_tokens, multi_line);
                let _ = tx.send(AiEvent::Completion { seq, result });
            });
    }

    fn spawn_server(&mut self, server: &PathBuf, model: &PathBuf, target: &Endpoint) -> Result<(), String> {
        if let Some(parent) = self.server_log.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let log = fs::File::create(&self.server_log)
            .map_err(|e| format!("AI log: {e}"))?;
        let log_err = log.try_clone().map_err(|e| format!("AI log: {e}"))?;
        let mut command = Command::new(server);
        if let Some(dir) = server.parent() {
            command.env("LD_LIBRARY_PATH", dir);
        }
        // Use the machine's cores (capped so a big host doesn't thrash) and a
        // larger prompt batch: prefill dominates latency on low-end CPUs.
        let threads = std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(2)
            .clamp(1, 4);
        // When there is room, pin the model in RAM. Otherwise the kernel can
        // evict its mmap between prompts and every completion has to read the
        // GGUF back from disk, which is far slower than the generation itself
        // on a modest machine.
        let model_bytes = fs::metadata(model).map(|meta| meta.len()).unwrap_or(0);
        let available_bytes = available_memory_kb().unwrap_or(0) * 1024;
        let pin_model = model_bytes > 0
            && available_bytes > model_bytes.saturating_add(512 * 1024 * 1024);
        command
            .arg("-m")
            .arg(model)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(target.port.to_string())
            .arg("-c")
            .arg("2048")
            .arg("-t")
            .arg(threads.to_string())
            .arg("-tb")
            .arg(threads.to_string())
            .arg("-b")
            .arg("128")
            .arg("-np")
            .arg("1")
            .arg("-ngl")
            .arg("0");
        if pin_model {
            command.arg("--mlock");
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err));
        let child = command
            .spawn()
            .map_err(|e| format!("Could not start llama-server: {e}"))?;
        self.server = Some(child);
        Ok(())
    }

    fn spawn_server_waiter(&self, port: u16) {
        let tx = self.tx.clone();
        let _ = thread::Builder::new()
            .name("kodemo-ai-server-wait".to_owned())
            .spawn(move || {
                let start = Instant::now();
                while start.elapsed() < Duration::from_secs(45) {
                    let ready = if port == 8090 {
                        health_ok("127.0.0.1", port)
                    } else {
                        endpoint_reachable("127.0.0.1", port)
                    };
                    if ready {
                        let _ = tx.send(AiEvent::ServerReady);
                        return;
                    }
                    thread::sleep(Duration::from_millis(450));
                }
                let _ = tx.send(AiEvent::ServerFailed(format!(
                    "llama-server didn't answer on 127.0.0.1:{port} within 45 seconds. See ~/.cache/kodemo/ai-server.log."
                )));
            });
    }

    fn stop_server(&mut self) {
        if let Some(mut child) = self.server.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[derive(Debug)]
struct Endpoint {
    host: String,
    port: u16,
    path: String,
}

impl Endpoint {
    fn parse(raw: &str) -> Result<Self, String> {
        let raw = raw.trim();
        let (host_port, path) = if let Some(rest) = raw.strip_prefix("http://") {
            split_host_path(rest)
        } else if raw.starts_with("https://") {
            return Err("HTTPS endpoints are not supported by the built-in client. Use http://localhost.".to_owned());
        } else {
            split_host_path(raw)
        };
        let (host, port) = match host_port.rsplit_once(':') {
            Some((host, port)) if !host.is_empty() => (
                host.to_owned(),
                port.parse::<u16>().map_err(|_| "Invalid port in AI endpoint.".to_owned())?,
            ),
            _ => (host_port.to_owned(), 80),
        };
        if host.is_empty() {
            return Err("Missing host in AI endpoint.".to_owned());
        }
        let path = if path.is_empty() || path == "/" {
            "/completion".to_owned()
        } else if path.starts_with('/') {
            path.to_owned()
        } else {
            format!("/{path}")
        };
        Ok(Self { host, port, path })
    }

    fn is_local(&self) -> bool {
        self.host == "127.0.0.1" || self.host == "localhost" || self.host == "::1"
    }

    fn reachable(&self) -> bool {
        endpoint_reachable(&self.host, self.port)
    }

    fn address(&self) -> String {
        if self.host.contains(':') && !self.host.starts_with('[') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    fn url(&self) -> String {
        format!("http://{}{}", self.address(), self.path)
    }
}

fn split_host_path(input: &str) -> (&str, &str) {
    match input.find('/') {
        Some(index) => (&input[..index], &input[index..]),
        None => (input, ""),
    }
}

fn health_ok(host: &str, port: u16) -> bool {
    let host = if host == "localhost" { "127.0.0.1" } else { host };
    let Ok(mut addrs) = (host, port).to_socket_addrs() else {
        return false;
    };
    let Some(addr) = addrs.next() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(800)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    let request = format!(
        "GET /health HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = Vec::with_capacity(512);
    let mut buffer = [0u8; 256];
    while response.len() < 1024 {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => response.extend_from_slice(&buffer[..n]),
            Err(_) => return false,
        }
    }
    let head = String::from_utf8_lossy(&response);
    head.starts_with("HTTP/1.1 200") || head.starts_with("HTTP/1.0 200")
}

fn endpoint_reachable(host: &str, port: u16) -> bool {
    let host = if host == "localhost" { "127.0.0.1" } else { host };
    let Ok(addrs) = (host, port).to_socket_addrs() else {
        return false;
    };
    for addr in addrs {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok() {
            return true;
        }
    }
    false
}

fn completion_request(
    endpoint: &str,
    model: &str,
    prompt: &str,
    max_tokens: usize,
    multi_line: bool,
) -> Result<String, String> {
    let target = Endpoint::parse(endpoint)?;
    // A single-line hint stops at the newline. A boosted request lets the
    // model finish the block, so only the FIM markers terminate it.
    let stops: Vec<&str> = if multi_line {
        vec![FIM_PREFIX, FIM_SUFFIX, "<|fim_pad|>", "<|endoftext|>", "<|file_sep|>"]
    } else {
        vec![
            "\n",
            FIM_PREFIX,
            FIM_SUFFIX,
            "<|fim_pad|>",
            "<|endoftext|>",
            "<|file_sep|>",
        ]
    };
    let body = if target.path.contains("api/generate") {
        json!({
            "model": model,
            "prompt": prompt,
            "raw": true,
            "stream": false,
            "options": {
                "num_predict": max_tokens,
                "temperature": 0.1,
                "top_p": 0.92,
                "top_k": 24,
                "min_p": 0.05,
                "repeat_penalty": 1.02,
                "stop": stops
            }
        })
    } else if target.path.contains("/v1/completions") || target.path.contains("openai") {
        json!({
            "model": model,
            "prompt": prompt,
            "max_tokens": max_tokens,
            "temperature": 0.1,
            "top_p": 0.92,
            "stream": false,
            "stop": stops
        })
    } else {
        json!({
            "prompt": prompt,
            "n_predict": max_tokens,
            "temperature": 0.1,
            "top_p": 0.92,
            "top_k": 24,
            "min_p": 0.05,
            "repeat_penalty": 1.02,
            "cache_prompt": true,
            "stream": false,
            "stop": stops
        })
    };
    let body = serde_json::to_string(&body).map_err(|e| format!("AI request JSON: {e}"))?;
    let (status, raw) = http_post(&target, body.as_bytes())?;
    if !(200..300).contains(&status) {
        let preview: String = raw.chars().take(240).collect();
        return Err(format!("AI server returned HTTP {status}: {preview}"));
    }
    let value: Value =
        serde_json::from_str(&raw).map_err(|e| format!("AI server returned invalid JSON: {e}"))?;
    completion_text(&value)
        .ok_or_else(|| "The AI response did not contain a completion.".to_owned())
}

fn completion_text(value: &Value) -> Option<String> {
    if let Some(text) = value.get("content").and_then(Value::as_str) {
        return Some(text.to_owned());
    }
    if let Some(text) = value.get("response").and_then(Value::as_str) {
        return Some(text.to_owned());
    }
    if let Some(text) = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("text"))
        .and_then(Value::as_str)
    {
        return Some(text.to_owned());
    }
    None
}

/// A tiny, language-correct header that anchors Qwen to the file type.
/// Start the FIM prefix at a line boundary, normally two lines above the
/// cursor. The anchor stays put while typing on the current line, which makes
/// llama.cpp's prompt cache effective on slow machines.
fn prefix_start(chars: &[char], cursor: usize, context_lines: usize, budget: usize) -> usize {
    let cursor = cursor.min(chars.len());
    let mut start = 0;
    let mut seen = 0;
    let mut i = cursor;
    while i > 0 {
        i -= 1;
        if chars[i] == '\n' {
            seen += 1;
            start = i + 1;
            if seen > context_lines {
                break;
            }
        }
    }
    if cursor.saturating_sub(start) > budget {
        // A very long line would make the prompt expensive; keep just that
        // line. Its start remains stable while the cursor moves within it.
        let mut line_start = 0;
        let mut i = cursor;
        while i > 0 {
            i -= 1;
            if chars[i] == '\n' {
                line_start = i + 1;
                break;
            }
        }
        start = if line_start > 0 {
            line_start
        } else {
            cursor.saturating_sub(budget)
        };
    }
    start
}

fn language_preamble(language: Language) -> Option<String> {
    let (open, close) = match language {
        Language::Html | Language::Markdown => ("<!-- ", " -->"),
        Language::Css => ("/* ", " */"),
        Language::Sql => ("-- ", ""),
        Language::Python | Language::Shell | Language::Toml | Language::Yaml => ("# ", ""),
        Language::Json | Language::Plain => return None,
        _ => ("// ", ""),
    };
    Some(format!(
        "{open}Kodemo file: {}{close}\n",
        language.display()
    ))
}

fn sanitize(raw: &str, max_lines: usize) -> Option<String> {
    let mut text = raw.replace("\r\n", "\n").replace('\r', "");
    for marker in [FIM_PREFIX, FIM_SUFFIX, FIM_MIDDLE, "<|fim_pad|>", "<|endoftext|>", "<|file_sep|>"] {
        if let Some(index) = text.find(marker) {
            text.truncate(index);
        }
    }
    // Ghost text has to stay readable over the document, so drop the language
    // header and Markdown fences small models like to add, and never show
    // more than the requested number of lines.
    let mut kept: Vec<String> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.contains("Kodemo file:") || trimmed.starts_with("```") {
            continue;
        }
        // Leading blank lines are just the model clearing its throat; once
        // real content started, blank lines keep the code's shape.
        if kept.is_empty() && trimmed.is_empty() {
            continue;
        }
        kept.push(line.trim_end_matches([' ', '\t']).to_owned());
        if kept.len() >= max_lines.max(1) {
            break;
        }
    }
    while kept.last().is_some_and(|line| line.trim().is_empty()) {
        kept.pop();
    }
    let first = kept.first_mut()?;
    *first = first.trim_start_matches('`').trim_start().to_owned();
    let out = kept.join("\n");
    if out.trim().is_empty() || out.chars().all(|c| !c.is_alphanumeric()) {
        None
    } else {
        Some(out)
    }
}

fn http_post(target: &Endpoint, body: &[u8]) -> Result<(u16, String), String> {
    let addrs: Vec<_> = (target.host.as_str(), target.port)
        .to_socket_addrs()
        .map_err(|e| format!("Could not resolve {}: {e}", target.host))?
        .collect();
    if addrs.is_empty() {
        return Err(format!("Could not resolve {}.", target.host));
    }
    let mut last_error = String::new();
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(2)) {
            Ok(mut stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(90)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
                let request = format!(
                    "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nAccept: application/json\r\n\r\n",
                    target.path,
                    target.address(),
                    body.len()
                );
                stream
                    .write_all(request.as_bytes())
                    .and_then(|_| stream.write_all(body))
                    .map_err(|e| format!("Could not write to the AI server: {e}"))?;
                let mut response = Vec::with_capacity(4096);
                stream
                    .read_to_end(&mut response)
                    .map_err(|e| format!("Could not read from the AI server: {e}"))?;
                return parse_http_response(&response);
            }
            Err(err) => last_error = err.to_string(),
        }
    }
    Err(format!(
        "The AI server is not reachable on {}: {last_error}",
        target.address()
    ))
}

fn parse_http_response(response: &[u8]) -> Result<(u16, String), String> {
    let split = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| "Malformed HTTP response from the AI server.".to_owned())?;
    let head = String::from_utf8_lossy(&response[..split]);
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| "Malformed HTTP status from the AI server.".to_owned())?;
    let body = &response[split + 4..];
    let chunked = head.to_ascii_lowercase().contains("transfer-encoding: chunked");
    let bytes = if chunked {
        decode_chunked(body)
    } else {
        body.to_vec()
    };
    Ok((status, String::from_utf8_lossy(&bytes).into_owned()))
}

fn decode_chunked(mut body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let Some(line_end) = body.windows(2).position(|w| w == b"\r\n") else {
            break;
        };
        let size_text = String::from_utf8_lossy(&body[..line_end]);
        let size_text = size_text.split(';').next().unwrap_or("").trim();
        let Ok(size) = usize::from_str_radix(size_text, 16) else {
            break;
        };
        body = &body[line_end + 2..];
        if size == 0 {
            break;
        }
        if body.len() < size {
            out.extend_from_slice(body);
            break;
        }
        out.extend_from_slice(&body[..size]);
        body = &body[size..];
        if body.starts_with(b"\r\n") {
            body = &body[2..];
        }
    }
    out
}

fn available_memory_kb() -> Option<u64> {
    let raw = fs::read_to_string("/proc/meminfo").ok()?;
    raw.lines().find_map(|line| {
        let rest = line.strip_prefix("MemAvailable:")?;
        rest.split_whitespace().next()?.parse::<u64>().ok()
    })
}

fn cache_path() -> PathBuf {
    let base = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".cache"));
    base.join("kodemo/ai-server.log")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fim_sanitizer_drops_markers() {
        let text = sanitize("println!(\"hi\");<|fim_pad|>garbage", 1).unwrap();
        assert_eq!(text, "println!(\"hi\");");
    }

    #[test]
    fn boosted_sanitizer_keeps_blocks() {
        let text = sanitize("let a = 1;\nlet b = 2;\n\nlet c = a + b;", 8).unwrap();
        assert_eq!(text, "let a = 1;\nlet b = 2;\n\nlet c = a + b;");
        let single = sanitize("let a = 1;\nlet b = 2;", 1).unwrap();
        assert_eq!(single, "let a = 1;");
    }

    #[test]
    fn language_preamble_uses_correct_comment_syntax() {
        assert_eq!(
            language_preamble(Language::Python).unwrap(),
            "# Kodemo file: Python\n"
        );
        assert_eq!(
            language_preamble(Language::Html).unwrap(),
            "<!-- Kodemo file: HTML -->\n"
        );
        assert!(language_preamble(Language::Json).is_none());
    }

    #[test]
    fn sanitizer_skips_language_header() {
        let text = sanitize("// Kodemo file: Rust\nlet x = 1;", 1).unwrap();
        assert_eq!(text, "let x = 1;");
    }

    #[test]
    fn prefix_starts_at_a_stable_line_boundary() {
        let chars: Vec<char> = "a\nb\nc\nd".chars().collect();
        assert_eq!(prefix_start(&chars, 7, 2, 120), 2);
        // Typing on the same line must not move the anchor.
        assert_eq!(prefix_start(&chars, 6, 2, 120), 2);
        // A boosted request has a larger context budget but this short
        // document has no earlier line to include.
        assert_eq!(prefix_start(&chars, 7, 4, 120), 2);
    }

    #[test]
    fn endpoint_defaults_to_completion() {
        let endpoint = Endpoint::parse("http://127.0.0.1:8090").unwrap();
        assert_eq!(endpoint.path, "/completion");
        assert_eq!(endpoint.port, 8090);
    }

    #[test]
    fn chunked_bodies_are_decoded() {
        assert_eq!(decode_chunked(b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n"), b"Wikipedia");
    }
}
