use crate::util::{escape_html, human_size, mime_for, safe_relative};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, SystemTime};

const LIVE_SCRIPT: &str = r#"
<script>
(function () {
  let last = null;
  async function ping() {
    try {
      const r = await fetch('/__kodemo_ping', { cache: 'no-store' });
      if (!r.ok) return;
      const data = await r.json();
      if (last !== null && last !== data.v) location.reload();
      last = data.v;
    } catch (_) {}
  }
  ping();
  setInterval(ping, 900);
})();
</script>
"#;

pub struct PreviewServer {
    pub port: u16,
    stop: Arc<AtomicBool>,
    root: Arc<RwLock<PathBuf>>,
}

impl PreviewServer {
    pub fn start(preferred_port: u16, root: PathBuf) -> Result<Self, String> {
        let mut last_error = String::new();
        let mut listener = None;
        let mut port = preferred_port;
        for candidate in preferred_port..preferred_port.saturating_add(24) {
            match TcpListener::bind(("127.0.0.1", candidate)) {
                Ok(bound) => {
                    port = candidate;
                    listener = Some(bound);
                    break;
                }
                Err(err) => last_error = format!("port {candidate}: {err}"),
            }
        }
        let listener = listener.ok_or_else(|| {
            format!(
                "Could not start the preview server on ports {preferred_port}–{}: {last_error}",
                preferred_port.saturating_add(23)
            )
        })?;
        listener
            .set_nonblocking(true)
            .map_err(|e| format!("preview listener: {e}"))?;

        let stop = Arc::new(AtomicBool::new(false));
        let version = Arc::new(AtomicU64::new(1));
        let root = Arc::new(RwLock::new(root));
        let server = Self {
            port,
            stop: stop.clone(),
            root: root.clone(),
        };

        let accept_stop = stop.clone();
        let accept_root = root.clone();
        let accept_version = version.clone();
        thread::Builder::new()
            .name("kodemo-preview-http".to_owned())
            .spawn(move || {
                while !accept_stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let root = accept_root.clone();
                            let version = accept_version.clone();
                            let _ = thread::Builder::new()
                                .name("kodemo-preview-request".to_owned())
                                .spawn(move || {
                                    let _ = handle_connection(stream, &root, &version);
                                });
                        }
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(35));
                        }
                        Err(_) => thread::sleep(Duration::from_millis(100)),
                    }
                }
            })
            .map_err(|e| format!("preview thread: {e}"))?;

        let watch_stop = stop.clone();
        let watch_root = root;
        thread::Builder::new()
            .name("kodemo-preview-watch".to_owned())
            .spawn(move || {
                let mut previous = tree_mtime(&watch_root.read().ok().map(|p| p.clone()).unwrap_or_default());
                while !watch_stop.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(700));
                    let current = tree_mtime(&watch_root.read().ok().map(|p| p.clone()).unwrap_or_default());
                    if current != previous {
                        previous = current;
                        version.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
            .map_err(|e| format!("preview watcher: {e}"))?;

        Ok(server)
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    pub fn set_root(&self, root: PathBuf) {
        if let Ok(mut guard) = self.root.write() {
            *guard = root;
        }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn root(&self) -> PathBuf {
        self.root.read().map(|p| p.clone()).unwrap_or_default()
    }
}

impl Drop for PreviewServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn tree_mtime(root: &Path) -> u64 {
    if root.as_os_str().is_empty() {
        return 0;
    }
    let mut newest = SystemTime::UNIX_EPOCH;
    let files = crate::fs_util::walk_files(root, 4096);
    for file in files {
        if let Ok(meta) = fs::metadata(&file) {
            if let Ok(modified) = meta.modified() {
                if modified > newest {
                    newest = modified;
                }
            }
        }
    }
    newest
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn handle_connection(
    mut stream: TcpStream,
    root: &Arc<RwLock<PathBuf>>,
    version: &Arc<AtomicU64>,
) -> std::io::Result<()> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let mut request = Vec::with_capacity(1024);
    let mut buffer = [0u8; 2048];
    loop {
        let n = stream.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..n]);
        if request.windows(4).any(|w| w == b"\r\n\r\n") || request.len() > 32 * 1024 {
            break;
        }
    }
    if request.is_empty() {
        return Ok(());
    }
    let head = String::from_utf8_lossy(&request);
    let first_line = head.lines().next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let target = parts.next().unwrap_or("/");
    let head_only = method == "HEAD";
    if method != "GET" && !head_only {
        respond(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            b"Method not allowed",
            head_only,
            false,
        )?;
        return Ok(());
    }

    if target.starts_with("/__kodemo_ping") {
        let body = format!("{{\"v\":{}}}", version.load(Ordering::Relaxed));
        return respond(
            &mut stream,
            "200 OK",
            "application/json; charset=utf-8",
            body.as_bytes(),
            head_only,
            false,
        );
    }

    let root_path = root.read().map(|r| r.clone()).unwrap_or_default();
    if root_path.as_os_str().is_empty() {
        return respond(
            &mut stream,
            "503 Service Unavailable",
            "text/plain; charset=utf-8",
            b"No workspace is open.",
            head_only,
            false,
        );
    }

    let request_path = target.split('?').next().unwrap_or("/");
    let Some(relative) = safe_relative(request_path) else {
        return respond(
            &mut stream,
            "400 Bad Request",
            "text/plain; charset=utf-8",
            b"Invalid path",
            head_only,
            false,
        );
    };
    let mut path = if relative.as_os_str().is_empty() {
        root_path.clone()
    } else {
        root_path.join(&relative)
    };
    if path.is_dir() {
        let index = path.join("index.html");
        if index.is_file() {
            path = index;
        } else {
            let body = directory_listing(&path, request_path);
            return respond(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                body.as_bytes(),
                head_only,
                false,
            );
        }
    }
    if !path.is_file() && path.extension().is_none() {
        let index = root_path.join("index.html");
        if index.is_file() {
            path = index;
        }
    }
    if !path.is_file() {
        let body = error_page(
            "404",
            "We couldn't find that file.",
            &format!("{} was not found in the preview folder.", request_path),
        );
        return respond(
            &mut stream,
            "404 Not Found",
            "text/html; charset=utf-8",
            body.as_bytes(),
            head_only,
            false,
        );
    }

    let canonical_root = root_path.canonicalize().unwrap_or(root_path.clone());
    let canonical_path = match path.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            return respond(
                &mut stream,
                "404 Not Found",
                "text/plain; charset=utf-8",
                b"File not found",
                head_only,
                false,
            );
        }
    };
    if !canonical_path.starts_with(&canonical_root) {
        return respond(
            &mut stream,
            "403 Forbidden",
            "text/plain; charset=utf-8",
            b"Access denied",
            head_only,
            false,
        );
    }

    let mime = mime_for(&canonical_path);
    let mut bytes = fs::read(&canonical_path)?;
    if mime.starts_with("text/html") {
        let html = String::from_utf8_lossy(&bytes).into_owned();
        bytes = inject_live_reload(&html).into_bytes();
    }
    respond(&mut stream, "200 OK", mime, &bytes, head_only, true)
}

fn inject_live_reload(html: &str) -> String {
    if let Some(index) = html.to_ascii_lowercase().rfind("</body>") {
        let mut out = String::with_capacity(html.len() + LIVE_SCRIPT.len());
        out.push_str(&html[..index]);
        out.push_str(LIVE_SCRIPT);
        out.push_str(&html[index..]);
        out
    } else {
        format!("{html}{LIVE_SCRIPT}")
    }
}

fn respond(
    stream: &mut TcpStream,
    status: &str,
    mime: &str,
    body: &[u8],
    head_only: bool,
    no_cache: bool,
) -> std::io::Result<()> {
    let cache = if no_cache {
        "no-store, no-cache, must-revalidate"
    } else {
        "no-store"
    };
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: {cache}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes())?;
    if !head_only {
        stream.write_all(body)?;
    }
    stream.flush()
}

fn directory_listing(dir: &Path, url: &str) -> String {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                return None;
            }
            let file_type = entry.file_type().ok()?;
            let meta = entry.metadata().ok();
            Some((name, file_type.is_dir(), meta.map(|m| m.len()).unwrap_or(0)))
        })
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let parent = if url == "/" {
        String::new()
    } else {
        "<tr><td><a href=\"../\">↩ ..</a></td><td></td></tr>".to_owned()
    };
    let base = if url.ends_with('/') {
        url.to_owned()
    } else {
        format!("{url}/")
    };
    let rows = entries
        .into_iter()
        .map(|(name, is_dir, size)| {
            let display = if is_dir {
                format!("{name}/")
            } else {
                name.clone()
            };
            let href = format!("{base}{}", urlencoding(&name));
            format!(
                "<tr><td><a href=\"{}\">{}</a></td><td>{}</td></tr>",
                escape_html(&href),
                escape_html(&display),
                if is_dir { String::new() } else { human_size(size) }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Kodemo preview</title><style>
:root{{color-scheme:dark}}*{{box-sizing:border-box}}body{{margin:0;background:#101418;color:#d7e2ea;font:14px/1.5 ui-sans-serif,system-ui,sans-serif}}
main{{max-width:880px;margin:8vh auto;padding:0 28px}}h1{{font-size:22px;font-weight:600;margin:0 0 6px}}p{{color:#7f8d98;margin:0 0 24px}}
table{{width:100%;border-collapse:collapse}}td{{padding:9px 12px;border-bottom:1px solid #22303a}}tr:hover td{{background:#172029}}
a{{color:#4fc3f7;text-decoration:none}}a:hover{{text-decoration:underline}}td:last-child{{text-align:right;color:#697985;white-space:nowrap}}
.badge{{display:inline-block;font-size:11px;letter-spacing:.08em;text-transform:uppercase;color:#78909c;border:1px solid #29404e;border-radius:99px;padding:3px 9px;margin-bottom:18px}}
</style></head><body><main><div class="badge">Kodemo · live preview</div><h1>{}</h1><p>{} entries</p><table>{}<tbody>{rows}</tbody></table></main></body></html>"#,
        escape_html(url),
        entries_len_hint(&rows),
        parent
    )
}

fn entries_len_hint(rows: &str) -> usize {
    rows.matches("<tr>").count()
}

fn error_page(code: &str, title: &str, detail: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{code} · Kodemo preview</title><style>body{{margin:0;min-height:100vh;display:grid;place-items:center;background:#101418;color:#d7e2ea;font:15px/1.6 system-ui,sans-serif}}main{{max-width:540px;padding:32px}}b{{color:#4fc3f7;font-size:13px;letter-spacing:.1em}}h1{{font-size:30px;margin:10px 0 8px}}p{{color:#8194a2}}</style></head><body><main><b>KODEMO PREVIEW</b><h1>{}</h1><p>{}</p></main></body></html>",
        escape_html(title),
        escape_html(detail)
    )
}

fn urlencoding(input: &str) -> String {
    let mut out = String::new();
    for byte in input.as_bytes() {
        let c = *byte as char;
        if c.is_ascii_alphanumeric() || "-._~/".contains(c) {
            out.push(c);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injection_lands_before_body_end() {
        let html = "<html><body><h1>Hi</h1></body></html>";
        let injected = inject_live_reload(html);
        assert!(injected.contains("__kodemo_ping"));
        assert!(injected.find("__kodemo_ping").unwrap() < injected.rfind("</body>").unwrap());
    }

    #[test]
    fn directory_html_is_escaped() {
        let text = urlencoding("hello world!.txt");
        assert_eq!(text, "hello%20world%21.txt");
    }
}
