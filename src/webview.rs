use crate::settings::home_dir;
use crate::util::child_window_id;
use eframe::egui::Rect;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

const XID_PREFIX: &str = "KODEMO_WEBVIEW_XID";

struct DisplayHandle {
    xlib: x11_dl::xlib::Xlib,
    display: *mut x11_dl::xlib::Display,
}

impl DisplayHandle {
    fn open() -> Result<Self, String> {
        let xlib = x11_dl::xlib::Xlib::open().map_err(|e| format!("libX11: {e}"))?;
        let display = unsafe { (xlib.XOpenDisplay)(std::ptr::null()) };
        if display.is_null() {
            return Err("could not connect to the X11 display".to_owned());
        }
        Ok(Self { xlib, display })
    }
}

impl Drop for DisplayHandle {
    fn drop(&mut self) {
        if !self.display.is_null() {
            unsafe {
                (self.xlib.XCloseDisplay)(self.display);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PreviewStatus {
    Idle,
    Starting,
    Ready,
    Failed(String),
}

pub struct PreviewWebView {
    child: Option<Child>,
    reader: Option<Receiver<Result<u64, String>>>,
    xid: Option<u64>,
    display: Option<DisplayHandle>,
    last_geometry: Option<(i32, i32, u32, u32)>,
    mapped: bool,
    pub url: String,
    pub software: bool,
    pub status: PreviewStatus,
    pub log_path: PathBuf,
    parent_id: u64,
    failed_url: Option<String>,
}

impl Default for PreviewWebView {
    fn default() -> Self {
        Self {
            child: None,
            reader: None,
            xid: None,
            display: None,
            last_geometry: None,
            mapped: false,
            url: String::new(),
            software: false,
            status: PreviewStatus::Idle,
            log_path: cache_log_path(),
            parent_id: 0,
            failed_url: None,
        }
    }
}

impl PreviewWebView {
    pub fn reload(&mut self) {
        self.stop();
        self.failed_url = None;
        self.status = PreviewStatus::Idle;
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.reader = None;
        self.xid = None;
        self.last_geometry = None;
        if self.mapped {
            self.mapped = false;
        }
        self.status = PreviewStatus::Idle;
    }

    /// Synchronize the native child window with the egui panel rectangle.
    /// `visible` is false while another tab, a menu, or a modal covers the preview.
    pub fn update(
        &mut self,
        frame: &eframe::Frame,
        rect: Option<Rect>,
        visible: bool,
        url: &str,
        pixels_per_point: f32,
        software: bool,
    ) {
        if self.software != software {
            self.software = software;
            if self.child.is_some() {
                self.reload();
            }
        }

        let parent = frame
            .window_handle()
            .ok()
            .and_then(|h| child_window_id(h.as_raw()));
        let Some(parent) = parent else {
            self.status = PreviewStatus::Failed(
                "The embedded preview needs an X11 window. Use “Open in browser” on Wayland."
                    .to_owned(),
            );
            return;
        };
        if parent != self.parent_id && self.child.is_some() {
            self.stop();
        }
        self.parent_id = parent;

        if !visible || rect.is_none() {
            self.unmap();
            return;
        }
        let rect = rect.unwrap();
        if rect.width() < 8.0 || rect.height() < 8.0 {
            self.unmap();
            return;
        }

        if self.url != url {
            self.url = url.to_owned();
            self.failed_url = None;
            if self.child.is_some() {
                self.stop();
            }
        }

        if self.child.is_none()
            && !url.is_empty()
            && self.failed_url.as_deref() != Some(url)
        {
            if let Err(err) = self.spawn(parent, rect, url, pixels_per_point) {
                self.failed_url = Some(url.to_owned());
                self.status = PreviewStatus::Failed(err);
                return;
            }
        }

        self.poll_reader();
        if let Some(child) = &mut self.child {
            if let Ok(Some(status)) = child.try_wait() {
                self.child = None;
                self.reader = None;
                self.xid = None;
                self.failed_url = Some(url.to_owned());
                self.status = PreviewStatus::Failed(format!(
                    "WebKitGTK exited before the preview was ready ({status}). See {}",
                    self.log_path.display()
                ));
                return;
            }
        }
        if let PreviewStatus::Failed(_) = &self.status {
            if self.child.is_none() {
                self.failed_url = Some(url.to_owned());
                return;
            }
        }

        let Some(xid) = self.xid else {
            if matches!(self.status, PreviewStatus::Idle) {
                self.status = PreviewStatus::Starting;
            }
            return;
        };
        let Some(display) = self.display.as_ref() else {
            return;
        };
        let geometry = (
            (rect.min.x * pixels_per_point).round() as i32,
            (rect.min.y * pixels_per_point).round() as i32,
            (rect.width() * pixels_per_point).round().max(1.0) as u32,
            (rect.height() * pixels_per_point).round().max(1.0) as u32,
        );
        unsafe {
            if self.last_geometry != Some(geometry) {
                let (x, y, w, h) = geometry;
                (display.xlib.XMoveResizeWindow)(display.display, xid, x, y, w, h);
                (display.xlib.XSync)(display.display, 0);
                self.last_geometry = Some(geometry);
            }
            if !self.mapped {
                (display.xlib.XMapRaised)(display.display, xid);
                (display.xlib.XSync)(display.display, 0);
                self.mapped = true;
            }
        }
        if self.status == PreviewStatus::Starting {
            self.status = PreviewStatus::Ready;
        }
    }

    fn unmap(&mut self) {
        if !self.mapped {
            return;
        }
        if let (Some(display), Some(xid)) = (self.display.as_ref(), self.xid) {
            unsafe {
                (display.xlib.XUnmapWindow)(display.display, xid);
                (display.xlib.XSync)(display.display, 0);
            }
        }
        self.mapped = false;
        self.last_geometry = None;
    }

    fn poll_reader(&mut self) {
        let mut messages = Vec::new();
        if let Some(reader) = &self.reader {
            while let Ok(message) = reader.try_recv() {
                messages.push(message);
            }
        }
        for message in messages {
            match message {
                Ok(xid) => {
                    self.xid = Some(xid);
                    self.status = PreviewStatus::Starting;
                }
                Err(err) => {
                    if self.status != PreviewStatus::Failed(err.clone()) {
                        self.status = PreviewStatus::Failed(err);
                    }
                }
            }
        }
    }

    fn spawn(
        &mut self,
        parent: u64,
        rect: Rect,
        url: &str,
        pixels_per_point: f32,
    ) -> Result<(), String> {
        let helper = locate_helper().ok_or_else(|| {
            format!(
                "Could not find the kodemo-webview helper. Build both binaries with `cargo build --bins` (looked next to {}).",
                std::env::current_exe()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "Kodemo".into())
            )
        })?;
        if self.display.is_none() {
            self.display = Some(DisplayHandle::open()?);
        }
        let _ = fs::create_dir_all(self.log_path.parent().unwrap_or(&home_dir()));
        let log = File::create(&self.log_path).map_err(|e| format!("preview log: {e}"))?;
        let log_err = log.try_clone().map_err(|e| format!("preview log: {e}"))?;
        let x = (rect.min.x * pixels_per_point).round() as i32;
        let y = (rect.min.y * pixels_per_point).round() as i32;
        let w = (rect.width() * pixels_per_point).round().max(1.0) as i32;
        let h = (rect.height() * pixels_per_point).round().max(1.0) as i32;
        let mut command = Command::new(&helper);
        command
            .arg(parent.to_string())
            .arg(x.to_string())
            .arg(y.to_string())
            .arg(w.to_string())
            .arg(h.to_string())
            .arg(url)
            .env("KODEMO_SOFTWARE", if self.software { "1" } else { "0" })
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(log_err));
        let mut child = command
            .spawn()
            .map_err(|e| format!("could not launch {}: {e}", helper.display()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "could not capture the WebView helper output".to_owned())?;
        let (tx, rx) = channel();
        let _log_for_thread = log;
        let log_path_for_thread = self.log_path.clone();
        thread::Builder::new()
            .name("kodemo-webview-reader".to_owned())
            .spawn(move || {
                let reader = BufReader::new(stdout);
                let mut found_xid = false;
                for line in reader.lines().map_while(Result::ok) {
                    if let Some(raw) = line.strip_prefix(XID_PREFIX) {
                        if let Ok(xid) = raw.trim().parse::<u64>() {
                            found_xid = true;
                            if tx.send(Ok(xid)).is_err() {
                                return;
                            }
                        }
                    }
                }
                if !found_xid {
                    let _ = tx.send(Err(format!(
                        "The WebView helper stopped before creating a window. See {}",
                        log_path_for_thread.display()
                    )));
                }
            })
            .map_err(|e| format!("preview thread: {e}"))?;
        self.child = Some(child);
        self.reader = Some(rx);
        self.status = PreviewStatus::Starting;
        Ok(())
    }
}

impl Drop for PreviewWebView {
    fn drop(&mut self) {
        self.stop();
    }
}

fn locate_helper() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("KODEMO_WEBVIEW_HELPER") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    let exe = std::env::current_exe().ok()?;
    let sibling = exe.with_file_name("kodemo-webview");
    if sibling.is_file() {
        return Some(sibling);
    }
    let mut candidates = vec![
        exe.parent()?.join("../libexec/kodemo/kodemo-webview"),
        exe.parent()?.join("kodemo-webview"),
    ];
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("target/debug/kodemo-webview"));
        candidates.push(cwd.join("target/release/kodemo-webview"));
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn cache_log_path() -> PathBuf {
    let base = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".cache"));
    base.join("kodemo/webview.log")
}

#[allow(dead_code)]
fn raw_handle_kind(handle: RawWindowHandle) -> &'static str {
    match handle {
        RawWindowHandle::Xlib(_) => "xlib",
        RawWindowHandle::Xcb(_) => "xcb",
        RawWindowHandle::Wayland(_) => "wayland",
        _ => "other",
    }
}

#[allow(dead_code)]
fn startup_delay() -> Duration {
    Duration::from_millis(100)
}
