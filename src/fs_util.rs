use crate::util::is_binary;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const MAX_TEXT_FILE: u64 = 32 * 1024 * 1024;

pub const IGNORED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "dist",
    "build",
    ".cache",
    ".venv",
    "venv",
    "__pycache__",
    ".idea",
    ".next",
    ".nuxt",
    "vendor",
];

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub hidden: bool,
    pub modified: Option<SystemTime>,
}

impl Entry {
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let meta = fs::symlink_metadata(path)?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        Ok(Self {
            path: path.to_path_buf(),
            hidden: name.starts_with('.'),
            name,
            is_dir: meta.is_dir(),
            modified: meta.modified().ok(),
        })
    }
}

pub fn list_dir(path: &Path, show_hidden: bool) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    let iter = fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))?;
    for entry in iter.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        if let Ok(item) = Entry::load(&entry.path()) {
            entries.push(item);
        }
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

pub fn read_text_file(path: &Path) -> Result<String, String> {
    let meta = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > MAX_TEXT_FILE {
        return Err(format!(
            "{} is too large to edit ({}).",
            path.display(),
            crate::util::human_size(meta.len())
        ));
    }
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if is_binary(&bytes) {
        return Err(format!("{} looks like a binary file.", path.display()));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn write_text_file(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let tmp = path.with_extension(format!(
        "{}.kodemo-tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("file")
    ));
    {
        let mut file = fs::File::create(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
        file.write_all(text.as_bytes())
            .map_err(|e| format!("{}: {e}", tmp.display()))?;
        let _ = file.sync_all();
    }
    if path.exists() {
        let _ = fs::set_permissions(&tmp, fs::metadata(path).map(|m| m.permissions()).unwrap_or_else(|_| fs::metadata(&tmp).unwrap().permissions()));
    }
    fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}

pub fn create_file(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(|_| ())
        .map_err(|e| format!("{}: {e}", path.display()))
}

pub fn create_folder(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))
}
/// Fast recursive file listing for quick-open and project search.
pub fn walk_files(root: &Path, max_files: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || IGNORED_DIRS.contains(&name.as_str()) {
                continue;
            }
            match entry.file_type() {
                Ok(ft) if ft.is_dir() => children.push(path),
                Ok(ft) if ft.is_file() => {
                    out.push(path);
                    if out.len() >= max_files {
                        return out;
                    }
                }
                _ => {}
            }
        }
        children.sort();
        stack.extend(children.into_iter().rev());
        if out.len() >= max_files {
            break;
        }
    }
    out
}

pub fn reveal_in_file_manager(path: &Path) {
    let parent = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
}

pub fn open_external(url_or_path: &str) {
    let _ = std::process::Command::new("xdg-open").arg(url_or_path).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_files_roundtrip() {
        let dir = std::env::temp_dir().join(format!("kodemo-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("note.txt");
        write_text_file(&path, "olá\nmundo").unwrap();
        assert_eq!(read_text_file(&path).unwrap(), "olá\nmundo");
        let _ = fs::remove_dir_all(&dir);
    }
}
