//! A deliberately small integrated terminal.
//!
//! Each command runs in its own shell process and streams output to the UI
//! through a channel, so a long build never blocks egui's render loop.

use crate::theme::ThemePalette;
use eframe::egui::{self, FontId, Key, Modifiers, RichText, ScrollArea, TextEdit, Ui};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};

const EXIT_MARKER: &str = "__KODEMO_EXIT__";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Command,
    Output,
    Error,
    Info,
    Success,
}

#[derive(Clone, Debug)]
pub struct TermLine {
    pub kind: LineKind,
    pub text: String,
}

enum TermEvent {
    Line(String),
    Finished(i32),
}

pub struct Terminal {
    pub lines: Vec<TermLine>,
    pub input: String,
    pub cwd: PathBuf,
    pub running: bool,
    history: Vec<String>,
    history_index: Option<usize>,
    rx: Option<Receiver<TermEvent>>,
    has_focus: bool,
}

impl Terminal {
    pub fn new(cwd: PathBuf) -> Self {
        let mut terminal = Self {
            lines: Vec::new(),
            input: String::new(),
            cwd,
            running: false,
            history: Vec::new(),
            history_index: None,
            rx: None,
            has_focus: false,
        };
        terminal.info("Kodemo terminal — commands run locally in the workspace folder.");
        terminal
    }

    pub fn set_cwd(&mut self, cwd: PathBuf) {
        if self.cwd != cwd {
            self.cwd = cwd;
            self.info(&format!("Workspace: {}", self.cwd.display()));
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.lines.push(TermLine {
            kind: LineKind::Info,
            text: text.into(),
        });
    }

    pub fn run_input(&mut self) {
        let command = self.input.trim().to_owned();
        if command.is_empty() || self.running {
            return;
        }
        self.input.clear();
        self.spawn(command);
    }

    /// Run a command chosen by the IDE (the Run button or a menu action).
    /// It is echoed like typed input and shares the same output stream.
    pub fn run_command(&mut self, command: impl Into<String>) {
        if self.running {
            self.info("A command is already running. Wait for it to finish or clear the terminal.");
            return;
        }
        self.spawn(command.into());
    }

    fn spawn(&mut self, command: String) {
        let command = command.trim().to_owned();
        if command.is_empty() {
            return;
        }
        self.history_index = None;
        self.history.retain(|item| item != &command);
        self.history.push(command.clone());
        self.lines.push(TermLine {
            kind: LineKind::Command,
            text: command.clone(),
        });
        let script = format!("{command} 2>&1\nprintf '\\n{EXIT_MARKER}%s\\n' \"$?\"");
        let mut process = Command::new("sh");
        process
            .arg("-lc")
            .arg(script)
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        match process.spawn() {
            Ok(mut child) => {
                let stdout = child.stdout.take();
                let (tx, rx) = channel();
                self.rx = Some(rx);
                self.running = true;
                let _ = std::thread::Builder::new()
                    .name("kodemo-terminal".to_owned())
                    .spawn(move || {
                        if let Some(stdout) = stdout {
                            let reader = BufReader::new(stdout);
                            for line in reader.lines().map_while(Result::ok) {
                                if let Some(code) = line.trim().strip_prefix(EXIT_MARKER) {
                                    let status = code.trim().parse::<i32>().unwrap_or(0);
                                    let _ = tx.send(TermEvent::Finished(status));
                                    return;
                                }
                                let _ = tx.send(TermEvent::Line(line));
                            }
                        }
                        let status = child.wait().map(|s| s.code().unwrap_or(1)).unwrap_or(1);
                        let _ = tx.send(TermEvent::Finished(status));
                    });
            }
            Err(err) => {
                self.lines.push(TermLine {
                    kind: LineKind::Error,
                    text: format!("Could not start shell: {err}"),
                });
            }
        }
    }

    pub fn poll(&mut self) {
        let mut events = Vec::new();
        if let Some(rx) = &self.rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            match event {
                TermEvent::Line(line) => self.lines.push(TermLine {
                    kind: LineKind::Output,
                    text: line,
                }),
                TermEvent::Finished(code) => {
                    self.running = false;
                    self.rx = None;
                    if code == 0 {
                        self.lines.push(TermLine {
                            kind: LineKind::Success,
                            text: "Process finished with exit code 0".to_owned(),
                        });
                    } else {
                        self.lines.push(TermLine {
                            kind: LineKind::Error,
                            text: format!("Process finished with exit code {code}"),
                        });
                    }
                }
            }
        }
        if self.lines.len() > 4000 {
            self.lines.drain(0..1000);
        }
    }

    pub fn show(&mut self, ui: &mut Ui, palette: &ThemePalette) {
        let input_height = 28.0;
        let body_height = (ui.available_height() - input_height - 4.0).max(40.0);
        ScrollArea::vertical()
            .id_salt("terminal-scroll")
            .stick_to_bottom(true)
            .max_height(body_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for line in &self.lines {
                    let color = match line.kind {
                        LineKind::Command => palette.fg,
                        LineKind::Output => palette.fg_muted,
                        LineKind::Error => palette.error,
                        LineKind::Info => palette.info,
                        LineKind::Success => palette.success,
                    };
                    let prefix = match line.kind {
                        LineKind::Command => "› ",
                        LineKind::Error => "  ",
                        _ => "  ",
                    };
                    ui.label(
                        RichText::new(format!("{prefix}{}", line.text))
                            .font(FontId::monospace(12.2))
                            .color(color),
                    );
                }
            });

        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let cwd = self
                .cwd
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("workspace");
            ui.label(
                RichText::new(format!("{cwd} ›"))
                    .font(FontId::monospace(12.0))
                    .color(palette.accent),
            );
            let response = ui.add(
                TextEdit::singleline(&mut self.input)
                    .id_salt("kodemo-terminal-input")
                    .font(FontId::monospace(12.5))
                    .desired_width(f32::INFINITY)
                    .hint_text(if self.running {
                        "Running…"
                    } else {
                        "Type a command and press Enter"
                    })
                    .margin(egui::Margin::symmetric(6, 4)),
            );
            self.has_focus = response.has_focus();
            if self.running {
                ui.spinner();
            }
            if response.has_focus() {
                let up = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp));
                let down = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown));
                if up || down {
                    if !self.history.is_empty() {
                        let next = match self.history_index {
                            None if up => Some(self.history.len().saturating_sub(1)),
                            Some(index) if up => Some(index.saturating_sub(1)),
                            Some(index) if down => {
                                if index + 1 >= self.history.len() {
                                    None
                                } else {
                                    Some(index + 1)
                                }
                            }
                            _ => None,
                        };
                        self.history_index = next;
                        self.input = next
                            .and_then(|index| self.history.get(index).cloned())
                            .unwrap_or_default();
                    }
                }
                let enter = ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
                if enter {
                    self.run_input();
                    response.request_focus();
                }
            }
        });
    }

    #[allow(dead_code)]
    pub fn focused(&self) -> bool {
        self.has_focus
    }
}
