use crate::settings::home_dir;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const INDEX: &str = include_str!("../assets/welcome/index.html");
const STYLES: &str = include_str!("../assets/welcome/styles.css");
const APP: &str = include_str!("../assets/welcome/app.js");
const README: &str = include_str!("../assets/welcome/README.md");
const SNIPPETS: &str = include_str!("../assets/welcome/snippets.py");

/// Kodemo's first-run workspace. Everything is a normal file the user can edit.
pub fn ensure_workspace() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".local/share"));
    let dir = base.join("kodemo/welcome");
    let _ = fs::create_dir_all(&dir);
    for (name, contents) in [
        ("index.html", INDEX),
        ("styles.css", STYLES),
        ("app.js", APP),
        ("README.md", README),
        ("snippets.py", SNIPPETS),
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
    for name in ["README.md", "snippets.py", "index.html", "app.js"] {
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

fn write(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(contents.as_bytes())
}

#[allow(dead_code)]
pub fn welcome_readme() -> PathBuf {
    ensure_workspace().join("README.md")
}
