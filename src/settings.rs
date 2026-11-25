use crate::theme::ThemePalette;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lang {
    #[serde(rename = "En", alias = "en", alias = "EN", alias = "English")]
    En,
    #[serde(rename = "Pt", alias = "pt", alias = "pt-BR", alias = "PtBr")]
    Pt,
}

impl Default for Lang {
    fn default() -> Self {
        let locale = std::env::var("LANG")
            .or_else(|_| std::env::var("LC_ALL"))
            .unwrap_or_default()
            .to_ascii_lowercase();
        if locale.starts_with("pt") {
            Self::Pt
        } else {
            Self::En
        }
    }
}

impl Lang {
    pub fn is_pt(self) -> bool {
        matches!(self, Self::Pt)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Pt => "Português (Brasil)",
        }
    }
}

fn default_theme() -> String {
    "Kodemo Dark".to_owned()
}

fn default_endpoint() -> String {
    "http://127.0.0.1:8090/completion".to_owned()
}

fn default_model() -> String {
    "qwen2.5-coder-0.5b-q4_k_m.gguf".to_owned()
}

/// What the Energy Saver button changed, so it can be restored when the mode
/// is switched off again.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyBackup {
    pub animations: bool,
    pub ghost: bool,
    pub theme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub theme: String,
    pub dark_mode: bool,
    pub font_size: f32,
    pub tab_size: usize,
    pub line_numbers: bool,
    pub animations: bool,
    pub auto_complete: bool,
    pub auto_close: bool,
    pub spellcheck: bool,
    pub completion_min_chars: usize,
    pub vsync: bool,
    pub unlimited_fps: bool,
    pub last_workspace: Option<String>,
    pub recent_files: Vec<String>,
    pub recent_folders: Vec<String>,
    pub preview_port: u16,
    pub show_hidden: bool,
    pub lang: Lang,
    pub ghost: bool,
    pub fullscreen: bool,
    pub maximized: bool,
    pub sidebar_width: f32,
    pub ai_enabled: bool,
    pub ai_endpoint: String,
    pub ai_model: String,
    pub ai_server_path: String,
    pub ai_model_path: String,
    /// Preferred download when no model is installed: `q4_k_m` (better) or
    /// `q4_0` (smaller and faster on very old CPUs).
    pub ai_model_quant: String,
    pub ai_auto_start: bool,
    pub ai_max_tokens: usize,
    pub completion_delay_ms: u64,
    pub hover_previews: bool,
    pub hover_audio: bool,
    pub terminal_height: f32,
    pub word_wrap: bool,
    pub minimap: bool,
    pub auto_save: bool,
    pub preview_auto_start: bool,
    pub preview_live_reload: bool,
    pub software_rendering: bool,
    pub energy_saver: bool,
    pub energy_backup: Option<EnergyBackup>,
    pub custom_theme: Option<ThemePalette>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            theme: default_theme(),
            dark_mode: true,
            font_size: 15.0,
            tab_size: 4,
            line_numbers: true,
            animations: true,
            auto_complete: true,
            auto_close: true,
            spellcheck: true,
            completion_min_chars: 2,
            vsync: true,
            unlimited_fps: false,
            last_workspace: None,
            recent_files: Vec::new(),
            recent_folders: Vec::new(),
            preview_port: 8080,
            show_hidden: false,
            lang: Lang::default(),
            ghost: true,
            fullscreen: false,
            maximized: true,
            sidebar_width: 236.0,
            ai_enabled: true,
            ai_endpoint: default_endpoint(),
            ai_model: default_model(),
            ai_server_path: String::new(),
            ai_model_path: String::new(),
            ai_model_quant: "q4_k_m".to_owned(),
            ai_auto_start: true,
            ai_max_tokens: 48,
            completion_delay_ms: 320,
            hover_previews: true,
            hover_audio: true,
            terminal_height: 176.0,
            word_wrap: false,
            minimap: true,
            auto_save: false,
            preview_auto_start: false,
            preview_live_reload: true,
            software_rendering: false,
            energy_saver: false,
            energy_backup: None,
            custom_theme: None,
        }
    }
}

impl Settings {
    pub fn config_dir() -> PathBuf {
        if let Ok(dir) = std::env::var("KODEMO_CONFIG_DIR") {
            return PathBuf::from(dir);
        }
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join(".config"));
        base.join("kodemo")
    }

    pub fn path() -> PathBuf {
        Self::config_dir().join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        let mut settings = match fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<Self>(&raw) {
                Ok(settings) => settings,
                Err(err) => {
                    let _ = fs::copy(&path, path.with_extension("json.bak"));
                    eprintln!("Kodemo: invalid settings ({err}); restored defaults");
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        };
        settings.normalize();
        settings
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(dir).map_err(|e| format!("create config folder: {e}"))?;
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| format!("serialize settings: {e}"))?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, raw).map_err(|e| format!("write settings: {e}"))?;
        fs::rename(&tmp, &path).map_err(|e| format!("replace settings: {e}"))?;
        Ok(())
    }

    pub fn normalize(&mut self) {
        self.font_size = self.font_size.clamp(10.0, 30.0);
        self.tab_size = self.tab_size.clamp(1, 12);
        self.completion_min_chars = self.completion_min_chars.clamp(1, 12);
        self.ai_max_tokens = self.ai_max_tokens.clamp(8, 256);
        if !matches!(self.ai_model_quant.as_str(), "q4_k_m" | "q4_0") {
            self.ai_model_quant = "q4_k_m".to_owned();
        }
        self.completion_delay_ms = self.completion_delay_ms.clamp(120, 5000);
        // 520 ms was the old default and felt sluggish on low-end CPUs.
        // Migrate it to the snappier default; explicit other values stay.
        if self.completion_delay_ms == 520 {
            self.completion_delay_ms = 320;
        }
        self.sidebar_width = self.sidebar_width.clamp(190.0, 520.0);
        self.terminal_height = self.terminal_height.clamp(100.0, 620.0);
        self.preview_port = self.preview_port.clamp(1024, 65500);
        self.version = 1;
        // Q4_K_M gives noticeably better answers than the old Q4_0 default,
        // and the model is already shipped next to it. Migrate configs that
        // still point at the previous default.
        if self.ai_model.trim() == "qwen2.5-coder-0.5b-q4_0.gguf" {
            self.ai_model = default_model();
        }
        if self.ai_model_path.ends_with("qwen2.5-coder-0.5b-q4_0.gguf") {
            let upgraded = self.ai_model_path.replace(
                "qwen2.5-coder-0.5b-q4_0.gguf",
                "qwen2.5-coder-0.5b-q4_k_m.gguf",
            );
            if Path::new(&upgraded).is_file() {
                self.ai_model_path = upgraded;
            }
        }
        if !crate::theme::theme_names().contains(&self.theme.as_str()) {
            self.theme = "Custom".to_owned();
        }
    }

    pub fn push_recent_file(&mut self, path: &Path) {
        let value = path.to_string_lossy().to_string();
        self.recent_files.retain(|p| p != &value);
        self.recent_files.insert(0, value);
        self.recent_files.truncate(16);
    }

    pub fn push_recent_folder(&mut self, path: &Path) {
        let value = path.to_string_lossy().to_string();
        self.recent_folders.retain(|p| p != &value);
        self.recent_folders.insert(0, value);
        self.recent_folders.truncate(12);
    }

    /// Known local Qwen model locations, model path first.
    pub fn find_model(&self) -> Option<PathBuf> {
        let candidates = [
            PathBuf::from(&self.ai_model_path),
            // Q4_K_M is the default: it answers noticeably more accurately
            // than Q4_0 at a similar prompt cost. The other quants stay
            // supported as fallbacks for existing installs.
            home_dir().join(".local/share/kodemo/models/qwen2.5-coder-0.5b-q4_k_m.gguf"),
            home_dir().join(".local/share/kodemo/models/qwen2.5-coder-0.5b-instruct-q4_k_m.gguf"),
            home_dir().join(".local/share/kodemo/models/qwen2.5-coder-0.5b-q4_0.gguf"),
            home_dir().join(".local/share/kodemo/models/qwen2.5-coder-0.5b-instruct-q4_0.gguf"),
            home_dir().join(".cache/kodemo/models/qwen2.5-coder-0.5b-q4_k_m.gguf"),
            home_dir().join(".cache/kodemo/models/qwen2.5-coder-0.5b-q4_0.gguf"),
            home_dir().join(".cache/kodemo/models/qwen2.5-coder-0.5b-instruct-q4_k_m.gguf"),
        ];
        candidates.into_iter().find(|p| !p.as_os_str().is_empty() && p.is_file())
    }

    /// Known llama-server locations.
    pub fn find_llama_server(&self) -> Option<PathBuf> {
        let candidates = [
            PathBuf::from(&self.ai_server_path),
            home_dir().join(".local/share/kodemo/llama/llama-server"),
            home_dir().join(".local/bin/llama-server"),
            PathBuf::from("/usr/lib/kodemo/llama/llama-server"),
            PathBuf::from("/usr/local/bin/llama-server"),
            PathBuf::from("/usr/bin/llama-server"),
        ];
        candidates
            .into_iter()
            .find(|p| !p.as_os_str().is_empty() && p.is_file())
            .or_else(|| {
                std::env::var("PATH").ok().and_then(|paths| {
                    std::env::split_paths(&paths)
                        .map(|p| p.join("llama-server"))
                        .find(|p| p.is_file())
                })
            })
    }
}

pub fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

pub fn format_relative(path: &Path) -> String {
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        if let Ok(rest) = path.strip_prefix(&home) {
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_ghost_delay_default_is_migrated() {
        let mut settings = Settings::default();
        settings.completion_delay_ms = 520;
        settings.normalize();
        assert_eq!(settings.completion_delay_ms, 320);
        settings.completion_delay_ms = 700;
        settings.normalize();
        assert_eq!(settings.completion_delay_ms, 700);
    }

    #[test]
    fn legacy_language_is_accepted() {
        let raw = r#"{"lang":"Pt","theme":"Gruvbox Dark","font_size":15.0}"#;
        let settings: Settings = serde_json::from_str(raw).unwrap();
        assert_eq!(settings.lang, Lang::Pt);
        assert!(!settings.ai_endpoint.is_empty());
        assert_eq!(settings.ai_model_quant, "q4_k_m");
    }

    #[test]
    fn unknown_model_quant_falls_back_to_q4_k_m() {
        let mut settings = Settings::default();
        settings.ai_model_quant = "q2".to_owned();
        settings.normalize();
        assert_eq!(settings.ai_model_quant, "q4_k_m");
        settings.ai_model_quant = "q4_0".to_owned();
        settings.normalize();
        assert_eq!(settings.ai_model_quant, "q4_0");
    }
}
