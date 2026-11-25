//! Native desktop file dialogs.
//!
//! On Linux the best available GTK file chooser is used through PyGObject,
//! which is part of a normal GTK desktop but does not require Kodemo itself to
//! link against GTK. If that is unavailable, `zenity` is used. The built-in
//! browser remains the fallback so the app still works on a minimal system.

use crate::file_picker::PickerMode;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::OnceLock;

const GTK_SCRIPT: &str = r#"
import sys
try:
    import gi
    gi.require_version("Gtk", "3.0")
    from gi.repository import Gtk
except Exception:
    sys.exit(3)
try:
    Gtk.init_check()
except Exception:
    pass
folder = sys.argv[1] == "folder"
title = sys.argv[2]
start = sys.argv[3] if len(sys.argv) > 3 else ""
action = Gtk.FileChooserAction.SELECT_FOLDER if folder else Gtk.FileChooserAction.OPEN
dialog = Gtk.FileChooserDialog(title=title, parent=None, action=action)
dialog.add_buttons(
    "Cancelar" if title.lower().startswith("abrir") else "Cancel",
    Gtk.ResponseType.CANCEL,
    "Abrir" if title.lower().startswith("abrir") else "Open",
    Gtk.ResponseType.ACCEPT,
)
dialog.set_default_size(1080, 680)
dialog.set_position(Gtk.WindowPosition.CENTER)
# The IDE does not expose a GTK window to use as a transient parent, so keep
# the chooser above the editor explicitly.
dialog.set_keep_above(True)
dialog.set_modal(True)
if start:
    try:
        dialog.set_current_folder(start)
    except Exception:
        pass
response = dialog.run()
if response == Gtk.ResponseType.ACCEPT:
    path = dialog.get_filename()
    if path:
        print(path, flush=True)
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativePickMode {
    OpenFile,
    OpenFolder,
}

impl NativePickMode {
    pub fn from_picker(mode: PickerMode) -> Option<Self> {
        match mode {
            PickerMode::OpenFile => Some(Self::OpenFile),
            PickerMode::OpenFolder => Some(Self::OpenFolder),
            PickerMode::SaveFile => None,
        }
    }

}

#[derive(Default)]
pub struct NativePicker {
    rx: Option<Receiver<Option<PathBuf>>>,
    mode: Option<NativePickMode>,
}

impl NativePicker {
    pub fn available() -> bool {
        static AVAILABLE: OnceLock<bool> = OnceLock::new();
        *AVAILABLE.get_or_init(|| {
            let gtk = Command::new("python3")
                .args([
                    "-c",
                    "import gi; gi.require_version('Gtk','3.0'); from gi.repository import Gtk",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false);
            gtk || which("zenity").is_some()
        })
    }

    pub fn open(
        &mut self,
        mode: NativePickMode,
        start: Option<PathBuf>,
        title: String,
    ) -> bool {
        if self.rx.is_some() || !Self::available() {
            return false;
        }
        let (tx, rx) = channel();
        let start = start.filter(|path| path.is_dir());
        std::thread::spawn(move || {
            let result = if gtk_available() {
                run_gtk(mode, start.as_deref(), &title)
            } else {
                run_zenity(mode, start.as_deref(), &title)
            };
            let _ = tx.send(result);
        });
        self.rx = Some(rx);
        self.mode = Some(mode);
        true
    }

    pub fn active(&self) -> bool {
        self.rx.is_some()
    }

    /// Returns `Some` once the dialog closes. The inner option is `None` when
    /// the user cancelled or the native backend failed.
    pub fn poll(&mut self) -> Option<(NativePickMode, Option<PathBuf>)> {
        let rx = self.rx.as_ref()?;
        let mode = self.mode?;
        match rx.try_recv() {
            Ok(path) => {
                self.rx = None;
                self.mode = None;
                Some((mode, path))
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.rx = None;
                self.mode = None;
                Some((mode, None))
            }
        }
    }
}

fn gtk_available() -> bool {
    static GTK: OnceLock<bool> = OnceLock::new();
    *GTK.get_or_init(|| {
        Command::new("python3")
            .args([
                "-c",
                "import gi; gi.require_version('Gtk','3.0'); from gi.repository import Gtk",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    })
}

fn run_gtk(mode: NativePickMode, start: Option<&std::path::Path>, title: &str) -> Option<PathBuf> {
    let action = match mode {
        NativePickMode::OpenFile => "file",
        NativePickMode::OpenFolder => "folder",
    };
    let output = Command::new("python3")
        .arg("-c")
        .arg(GTK_SCRIPT)
        .arg(action)
        .arg(title)
        .arg(start.map(|p| p.display().to_string()).unwrap_or_default())
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let path = text.lines().last()?.trim();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn run_zenity(mode: NativePickMode, start: Option<&std::path::Path>, title: &str) -> Option<PathBuf> {
    let mut command = Command::new("zenity");
    command.arg("--file-selection").arg(format!("--title={title}"));
    if mode == NativePickMode::OpenFolder {
        command.arg("--directory");
    }
    if let Some(start) = start {
        command.arg(format!("--filename={}/", start.display()));
    }
    let output = command
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let path = text.lines().last()?.trim();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn which(program: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(program))
            .find(|path| path.is_file())
    })
}
