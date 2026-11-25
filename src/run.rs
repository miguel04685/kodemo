//! Project-aware Run tasks.
//!
//! Pressing Run saves the open documents, then either starts the built-in live
//! preview (for static HTML/CSS) or sends the compile/run command for the
//! active project straight to the integrated terminal. Detection prefers
//! project manifests (`Cargo.toml`, `package.json`, `go.mod`, …) so the command
//! matches the project rather than just the file in focus.

use crate::syntax::Language;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunAction {
    /// Static web content: show the embedded preview instead of a shell task.
    Preview,
    /// Shell command executed with the project root as the working directory.
    Command(String),
}

pub fn plan(root: &Path, active: Option<&Path>) -> Option<RunAction> {
    let language = active.map(Language::from_path);
    let active = active.filter(|path| path.starts_with(root));
    let file = active.and_then(|path| path.strip_prefix(root).ok());

    // Static pages are better served by Kodemo's live-reload preview than by a
    // shell command. Plain CSS files use the project's index.html.
    if matches!(language, Some(Language::Html | Language::Css)) && has_static_index(root) {
        return Some(RunAction::Preview);
    }

    if let Some(script) = node_script(root) {
        return Some(RunAction::Command(script));
    }
    if root.join("Cargo.toml").is_file() {
        return Some(RunAction::Command("cargo run".to_owned()));
    }
    if root.join("go.mod").is_file() {
        return Some(RunAction::Command("go run .".to_owned()));
    }
    if root.join("manage.py").is_file() {
        return Some(RunAction::Command("python3 manage.py runserver".to_owned()));
    }
    if let Some(entry) = python_entry(root) {
        return Some(RunAction::Command(format!(
            "python3 -u {}",
            quote(&entry)
        )));
    }
    if root.join("pom.xml").is_file() {
        return Some(RunAction::Command(
            "mvn -q compile exec:java".to_owned(),
        ));
    }
    if root.join("build.gradle").is_file() || root.join("build.gradle.kts").is_file() {
        return Some(RunAction::Command("./gradlew run".to_owned()));
    }
    if root.join("composer.json").is_file() {
        if let Some(entry) = first_existing(root, &["public/index.php", "index.php"]) {
            return Some(RunAction::Command(format!(
                "php -S 127.0.0.1:8000 -t {}",
                quote(entry.parent().unwrap_or(root))
            )));
        }
    }
    if root.join("Makefile").is_file() {
        return Some(RunAction::Command("make".to_owned()));
    }

    if matches!(language, Some(Language::Html | Language::Css)) {
        return Some(RunAction::Preview);
    }

    let file = file?;
    let language = language?;
    let relative = quote(file);
    let command = match language {
        Language::Rust => {
            let output = std::env::temp_dir().join(format!("kodemo-run-{}", std::process::id()));
            format!(
                "rustc {} -O -o {} && {}",
                relative,
                quote(&output),
                quote(&output)
            )
        }
        Language::Python => format!("python3 -u {relative}"),
        Language::JavaScript => format!("node {relative}"),
        Language::TypeScript => {
            let local = root.join("node_modules/.bin/tsx");
            if local.is_file() {
                format!("./node_modules/.bin/tsx {relative}")
            } else {
                format!("node --experimental-strip-types {relative}")
            }
        }
        Language::C => {
            let output = std::env::temp_dir().join(format!("kodemo-run-{}", std::process::id()));
            format!(
                "cc {} -O2 -o {} -lm && {}",
                relative,
                quote(&output),
                quote(&output)
            )
        }
        Language::Cpp => {
            let output = std::env::temp_dir().join(format!("kodemo-run-{}", std::process::id()));
            format!(
                "c++ {} -O2 -o {} -lm && {}",
                relative,
                quote(&output),
                quote(&output)
            )
        }
        Language::Php => format!("php {relative}"),
        Language::Java => {
            let class = file
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("Main")
                .to_owned();
            let class_dir = file
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            format!(
                "javac {} && java -cp {} {class}",
                relative,
                quote(class_dir)
            )
        }
        Language::Go => format!("go run {relative}"),
        Language::Shell => format!("bash {relative}"),
        Language::Sql
        | Language::Json
        | Language::Markdown
        | Language::Toml
        | Language::Yaml
        | Language::Plain => return None,
        Language::Html | Language::Css => return Some(RunAction::Preview),
    };
    Some(RunAction::Command(command))
}

/// Compile/syntax-check the active file without running it.
/// Python is interpreted at runtime, so its compile task uses `py_compile`.
pub fn compile_plan(root: &Path, active: Option<&Path>) -> Option<String> {
    let active = active?;
    let language = Language::from_path(active);
    let file = active.strip_prefix(root).ok()?;
    if file.as_os_str().is_empty() {
        return None;
    }
    let relative = quote(file);
    Some(match language {
        Language::Rust => {
            if root.join("Cargo.toml").is_file() {
                "cargo check".to_owned()
            } else {
                let out = std::env::temp_dir()
                    .join(format!("kodemo-check-{}.rmeta", std::process::id()));
                format!(
                    "rustc --edition 2021 --emit=metadata -o {} {}",
                    quote(&out),
                    relative
                )
            }
        }
        Language::Python => format!("python3 -m py_compile {relative}"),
        Language::JavaScript => format!("node --check {relative}"),
        Language::TypeScript => {
            if root.join("tsconfig.json").is_file() {
                "npx --no-install tsc --noEmit".to_owned()
            } else {
                format!("npx --no-install tsc --noEmit {relative}")
            }
        }
        Language::C => format!("cc -fsyntax-only {relative}"),
        Language::Cpp => format!("c++ -fsyntax-only {relative}"),
        Language::Php => format!("php -l {relative}"),
        Language::Java => {
            let output = std::env::temp_dir()
                .join(format!("kodemo-java-{}", std::process::id()));
            format!(
                "mkdir -p {} && javac -d {} {}",
                quote(&output),
                quote(&output),
                relative
            )
        }
        Language::Go => {
            if root.join("go.mod").is_file() {
                "go build ./...".to_owned()
            } else {
                let out = std::env::temp_dir()
                    .join(format!("kodemo-check-{}", std::process::id()));
                format!("go build -o {} {}", quote(&out), relative)
            }
        }
        Language::Shell => format!("bash -n {relative}"),
        Language::Html
        | Language::Css
        | Language::Sql
        | Language::Json
        | Language::Markdown
        | Language::Toml
        | Language::Yaml
        | Language::Plain => return None,
    })
}

fn has_static_index(root: &Path) -> bool {
    root.join("index.html").is_file() || root.join("index.htm").is_file()
}

fn python_entry(root: &Path) -> Option<String> {
    first_existing(root, &["main.py", "app.py", "run.py", "server.py"])
        .map(|path| path.to_string_lossy().to_string())
}

fn first_existing(root: &Path, names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
}

fn node_script(root: &Path) -> Option<String> {
    let raw = fs::read_to_string(root.join("package.json")).ok()?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    let scripts = value.get("scripts")?;
    for (key, command) in [("start", "npm start"), ("dev", "npm run dev")] {
        if scripts.get(key).and_then(Value::as_str).is_some() {
            return Some(command.to_owned());
        }
    }
    None
}

/// POSIX shell single-quote so paths with spaces or apostrophes stay safe.
fn quote(path: impl AsRef<Path>) -> String {
    let text = path.as_ref().to_string_lossy();
    let escaped = text.replace('\'', "'\\''");
    format!("'{escaped}'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kodemo-run-test-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn cargo_project_runs_cargo() {
        let dir = fixture("cargo");
        fs::write(dir.join("Cargo.toml"), "[package]").unwrap();
        assert_eq!(
            plan(&dir, Some(&dir.join("src/main.rs"))),
            Some(RunAction::Command("cargo run".to_owned()))
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn active_python_file_runs_with_python3() {
        let dir = fixture("python");
        let file = dir.join("hello world.py");
        fs::write(&file, "print('hi')").unwrap();
        assert_eq!(
            plan(&dir, Some(&file)),
            Some(RunAction::Command(
                "python3 -u 'hello world.py'".to_owned()
            ))
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn static_html_uses_preview() {
        let dir = fixture("web");
        fs::write(dir.join("index.html"), "<html></html>").unwrap();
        assert_eq!(
            plan(&dir, Some(&dir.join("index.html"))),
            Some(RunAction::Preview)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn node_start_script_wins_over_file() {
        let dir = fixture("node");
        fs::write(
            dir.join("package.json"),
            r#"{"scripts":{"start":"vite"}}"#,
        )
        .unwrap();
        assert_eq!(
            plan(&dir, Some(&dir.join("app.js"))),
            Some(RunAction::Command("npm start".to_owned()))
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn python_compile_uses_py_compile() {
        let dir = fixture("python-compile");
        let file = dir.join("main.py");
        fs::write(&file, "print('hi')").unwrap();
        assert_eq!(
            compile_plan(&dir, Some(&file)),
            Some("python3 -m py_compile 'main.py'".to_owned())
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn html_has_no_compile_step() {
        let dir = fixture("web-compile");
        let file = dir.join("index.html");
        fs::write(&file, "<html></html>").unwrap();
        assert_eq!(compile_plan(&dir, Some(&file)), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn quotes_are_escaped() {
        assert_eq!(quote(Path::new("it's here.rs")), "'it'\\''s here.rs'");
    }
}
