//! The first-run workspace bundled with Kodemo.
//!
//! The welcome project is a normal static site: plain HTML, an inline
//! stylesheet/script, the Kodemo icon and the Kodemi mascot. Everything is
//! written as a regular file the user can inspect, edit and delete.

use crate::settings::home_dir;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const INDEX: &str = include_str!("../assets/welcome/index.html");
const ICON: &str = include_str!("../assets/welcome/icon.svg");
const KODEMI: &[u8] = include_bytes!("../assets/welcome/kodemi.png");
const FAVICON: &[u8] = include_bytes!("../assets/welcome/favicon.ico");

/// Kodemo's first-run workspace. Everything is a normal file the user can edit.
pub fn ensure_workspace() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".local/share"));
    let dir = base.join("kodemo/welcome");
    let _ = fs::create_dir_all(&dir);
    for (name, contents) in [
        ("index.html", INDEX.as_bytes()),
        ("icon.svg", ICON.as_bytes()),
        ("kodemi.png", KODEMI),
        ("favicon.ico", FAVICON),
    ] {
        let path = dir.join(name);
        if !path.exists() {
            let _ = write(&path, contents);
        }
    }
    migrate_shortcuts(&dir);
    dir
}

/// Older welcome workspaces were written before the preview shortcut changed.
/// Rewrite only the stale shortcut, leaving any other user edits untouched.
fn migrate_shortcuts(dir: &std::path::Path) {
    for name in ["index.html", "icon.svg"] {
        let path = dir.join(name);
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        if contents.contains("Ctrl+Shift+V") {
            let updated = contents.replace("Ctrl+Shift+V", "Ctrl+Alt+P");
            let _ = fs::write(path, updated);
        }
    }
}

fn write(path: &std::path::Path, contents: &[u8]) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(contents)
}
