use crate::ai::{AiEngine, AiStatus};
use crate::assist;
use crate::explorer::{Explorer, ExplorerAction};
use crate::file_picker::{FilePicker, PickerMode};
use crate::fs_util;
use crate::i18n::{t, theme_name};
use crate::icons::{self, Icon};
use crate::native_picker::{NativePickMode, NativePicker};
use crate::preview::HoverPreview;
use crate::preview_server::PreviewServer;
use crate::run::{self, RunAction};
use crate::settings::{format_relative, home_dir, EnergyBackup, Lang, Settings};
use crate::spell;
use crate::syntax::{self, Language};
use crate::system::{HardwareInfo, ProcSampler};
use crate::templates;
use crate::terminal::Terminal;
use crate::theme::{self, ThemePalette};
use crate::util;
use crate::webview::{PreviewStatus, PreviewWebView};
use crate::welcome;
use eframe::egui::{
    self, Align, Align2, CentralPanel, Color32, ComboBox, CornerRadius, FontId, Frame, Id, Key,
    Layout, Margin, Modal, Modifiers, Pos2, RichText, ScrollArea, Sense, SidePanel, Stroke,
    StrokeKind, TextEdit, TopBottomPanel, Ui, Vec2,
};
use egui::text::{CCursor, CCursorRange};
use egui::widgets::text_edit::TextEditState;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Activity {
    Explorer,
    Search,
    Git,
    Extensions,
    Ai,
}

impl Activity {
    fn title(self, lang: Lang) -> &'static str {
        match self {
            Self::Explorer => t(lang, "EXPLORER", "EXPLORADOR"),
            Self::Search => t(lang, "SEARCH", "PESQUISAR"),
            Self::Git => t(lang, "SOURCE CONTROL", "CONTROLE DE CÓDIGO"),
            Self::Extensions => t(lang, "EXTENSIONS", "EXTENSÕES"),
            Self::Ai => t(lang, "KODEMO AI", "KODEMO IA"),
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::Explorer => Icon::Files,
            Self::Search => Icon::Search,
            Self::Git => Icon::Branch,
            Self::Extensions => Icon::Extensions,
            Self::Ai => Icon::Sparkle,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PanelTab {
    Problems,
    Output,
    Terminal,
}

#[derive(Clone, Debug, PartialEq)]
enum Command {
    NewFile,
    OpenFile,
    OpenFolder,
    Save,
    SaveAs,
    SaveAll,
    CloseTab,
    CloseOthers,
    CloseAll,
    CloseProject,
    CompileProject,
    OpenSettings,
    OpenAbout,
    OpenHelp,
    QuickOpen,
    CommandPalette,
    FindInFiles,
    GoToLine,
    ToggleSidebar,
    TogglePanel,
    TogglePreview,
    ToggleTerminal,
    ToggleWordWrap,
    ToggleComment,
    ToggleMinimap,
    ToggleGhost,
    ToggleAi,
    ToggleEnergySaver,
    StartPreview,
    ReloadPreview,
    RunProject,
    OpenInBrowser,
    RestartAi,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ToggleFullscreen,
    NewTerminal,
    ClearTerminal,
    CopyPath,
    RevealActive,
    OpenRecentFile(String),
    OpenRecentFolder(String),
    ForgetRecent(String, bool),
    ClearRecent,
    SetActivity(Activity),
    Theme(String),
    Exit,
}

#[derive(Clone, Debug)]
enum ExplorerIntent {
    Action(ExplorerAction),
}

struct Document {
    id: u64,
    path: Option<PathBuf>,
    title: String,
    text: String,
    saved_text: String,
    language: Language,
    cursor_char: usize,
    pending_cursor: Option<usize>,
    last_edit: Instant,
}

impl Document {
    fn modified(&self) -> bool {
        self.text != self.saved_text
    }
}

struct HighlightCache {
    doc_id: u64,
    text_hash: u64,
    wrap_width: f32,
    galley: std::sync::Arc<egui::Galley>,
}

struct CompletionState {
    open: bool,
    doc_id: u64,
    start: usize,
    prefix: String,
    text_hash: u64,
    items: Vec<assist::CompletionItem>,
    selected: usize,
    anchor: Option<Pos2>,
    bounds: egui::Rect,
    suppress: Option<(u64, u64, usize)>,
}

impl Default for CompletionState {
    fn default() -> Self {
        Self {
            open: false,
            doc_id: 0,
            start: 0,
            prefix: String::new(),
            text_hash: 0,
            items: Vec::new(),
            selected: 0,
            anchor: None,
            bounds: egui::Rect::NOTHING,
            suppress: None,
        }
    }
}

impl CompletionState {
    fn close(&mut self) {
        self.open = false;
        self.items.clear();
        self.prefix.clear();
        self.anchor = None;
    }
}

struct SpellCache {
    doc_id: u64,
    text_hash: u64,
    language: Language,
    issues: Vec<spell::Issue>,
}

#[derive(Clone, Debug)]
struct SearchHit {
    path: PathBuf,
    line: usize,
    column: usize,
    text: String,
}

struct SearchState {
    query: String,
    case_sensitive: bool,
    whole_word: bool,
    hits: Vec<SearchHit>,
    searching: bool,
    rx: Option<Receiver<Vec<SearchHit>>>,
    last_started: String,
    focus: bool,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            case_sensitive: false,
            whole_word: false,
            hits: Vec::new(),
            searching: false,
            rx: None,
            last_started: String::new(),
            focus: false,
        }
    }
}

#[derive(Default)]
struct GitState {
    branch: String,
    changes: Vec<(String, String)>,
    error: Option<String>,
    loaded_for: Option<PathBuf>,
}

struct QuickOpen {
    open: bool,
    files_mode: bool,
    query: String,
    selected: usize,
    files: Vec<PathBuf>,
    just_opened: bool,
}

impl Default for QuickOpen {
    fn default() -> Self {
        Self {
            open: false,
            files_mode: false,
            query: String::new(),
            selected: 0,
            files: Vec::new(),
            just_opened: false,
        }
    }
}

#[derive(Clone, Debug)]
enum PromptKind {
    NewFile(PathBuf),
    NewFolder(PathBuf),
    Rename(PathBuf),
    GoToLine,
}

struct PromptDialog {
    kind: PromptKind,
    title: String,
    value: String,
    error: Option<String>,
    just_opened: bool,
}

#[derive(Clone, Debug)]
enum ConfirmKind {
    CloseTab(usize),
    Delete(PathBuf),
    CloseApp,
    CloseProject,
}

struct ConfirmDialog {
    kind: ConfirmKind,
    message: String,
}

enum StatusKind {
    Info,
    Success,
    Warning,
    Error,
}

struct StatusMessage {
    text: String,
    kind: StatusKind,
    at: Instant,
}

pub struct KodemoApp {
    settings: Settings,
    palette: ThemePalette,
    explorer: Explorer,
    docs: Vec<Document>,
    active: usize,
    next_doc_id: u64,
    activity: Activity,
    show_sidebar: bool,
    sidebar_resizing: bool,
    show_panel: bool,
    show_preview: bool,
    panel_tab: PanelTab,
    settings_open: bool,
    settings_snapshot: Option<Settings>,
    about_open: bool,
    help_open: bool,
    quick: QuickOpen,
    picker: FilePicker,
    native_picker: NativePicker,
    prompt: Option<PromptDialog>,
    confirm: Option<ConfirmDialog>,
    search: SearchState,
    git: GitState,
    terminal: Terminal,
    output: Vec<String>,
    status: Option<StatusMessage>,
    pending_command: Option<Command>,
    pending_explorer: Option<ExplorerIntent>,
    pending_open: Option<(PathBuf, Option<usize>)>,
    pending_close_tab: Option<usize>,
    highlight_cache: Option<HighlightCache>,
    spell_cache: Option<SpellCache>,
    completion: CompletionState,
    hover_preview: HoverPreview,
    ai: AiEngine,
    ai_due: Option<Instant>,
    ghost_controls_rect: Option<egui::Rect>,
    hardware: HardwareInfo,
    proc_stats: ProcSampler,
    banner: Option<egui::TextureHandle>,
    preview: PreviewWebView,
    preview_server: Option<PreviewServer>,
    preview_error: Option<String>,
    preview_url: String,
    preview_url_input: String,
    preview_target_doc: Option<u64>,
    pending_preview_rect: Option<egui::Rect>,
    software: bool,
    logo: Option<egui::TextureHandle>,
    applied_font_size: f32,
    window_title: String,
    settings_dirty: bool,
    last_settings_save: Instant,
    allow_close: bool,
    close_requested_handled: bool,
    view_initialized: bool,
    started: Instant,
}

impl KodemoApp {
    pub fn new(cc: &eframe::CreationContext<'_>, settings: Settings, software: bool) -> Self {
        let ctx = &cc.egui_ctx;
        theme::install_fonts(ctx, settings.font_size, settings.font_size * 0.92);
        let palette = theme::resolve(&settings);
        theme::apply_style(ctx, &settings, &palette);
        let logo = load_logo(ctx);
        let welcome_workspace = welcome::ensure_workspace();
        let first_run = settings
            .last_workspace
            .as_ref()
            .map(|p| !Path::new(p).is_dir())
            .unwrap_or(true);
        let workspace = settings
            .last_workspace
            .as_ref()
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| welcome_workspace.clone());
        let mut explorer = Explorer::default();
        explorer.set_root(Some(workspace.clone()));
        explorer.set_show_hidden(settings.show_hidden);
        let preview_auto_start = settings.preview_auto_start;
        let ai = AiEngine::new(&settings);
        let terminal = Terminal::new(workspace.clone());
        let mut app = Self {
            palette,
            explorer,
            docs: Vec::new(),
            active: 0,
            next_doc_id: 1,
            activity: Activity::Explorer,
            show_sidebar: true,
            sidebar_resizing: false,
            show_panel: false,
            show_preview: first_run,
            panel_tab: PanelTab::Terminal,
            settings_open: false,
            settings_snapshot: None,
            about_open: false,
            help_open: false,
            quick: QuickOpen::default(),
            picker: FilePicker::default(),
            native_picker: NativePicker::default(),
            prompt: None,
            confirm: None,
            search: SearchState::default(),
            git: GitState::default(),
            terminal,
            output: Vec::new(),
            status: None,
            pending_command: None,
            pending_explorer: None,
            pending_open: None,
            pending_close_tab: None,
            highlight_cache: None,
            spell_cache: None,
            completion: CompletionState::default(),
            hover_preview: HoverPreview::default(),
            ai,
            ai_due: None,
            ghost_controls_rect: None,
            hardware: HardwareInfo::detect(),
            proc_stats: ProcSampler::default(),
            banner: load_banner(ctx),
            preview: PreviewWebView::default(),
            preview_server: None,
            preview_error: None,
            preview_url: String::new(),
            preview_url_input: String::new(),
            preview_target_doc: None,
            pending_preview_rect: None,
            software,
            logo,
            applied_font_size: settings.font_size,
            window_title: String::new(),
            settings_dirty: false,
            last_settings_save: Instant::now(),
            allow_close: false,
            close_requested_handled: false,
            view_initialized: false,
            started: Instant::now(),
            settings,
        };
        app.log(format!("Workspace: {}", workspace.display()));
        app.log(format!(
            "Kodemo {} · {} · {} rendering",
            env!("CARGO_PKG_VERSION"),
            if app.settings.lang.is_pt() { "Português" } else { "English" },
            if software { "software" } else { "hardware" }
        ));
        if first_run {
            app.log("Welcome workspace created. Everything here is editable.");
            for name in ["index.html", "styles.css", "app.js", "README.md"] {
                let path = workspace.join(name);
                if path.is_file() {
                    app.open_path(path, None, false);
                }
            }
            if let Some(index) = app
                .docs
                .iter()
                .position(|doc| doc.title == "index.html")
            {
                app.active = index;
            }
            app.show_preview = true;
            app.ensure_preview();
            app.sync_preview_target(true);
        } else {
            app.sync_preview_target(true);
            if preview_auto_start {
                app.show_preview = true;
                app.ensure_preview();
            }
            let candidates = ["index.html", "README.md"];
            for name in candidates {
                let path = workspace.join(name);
                if path.is_file() {
                    app.open_path(path, None, false);
                    break;
                }
            }
            if app.docs.is_empty() {
                if let Some(recent) = app
                    .settings
                    .recent_files
                    .iter()
                    .map(PathBuf::from)
                    .find(|p| p.is_file() && p.starts_with(&workspace))
                {
                    app.open_path(recent, None, false);
                }
            }
        }
        app.sync_preview_target(true);
        app
    }

    fn lang(&self) -> Lang {
        self.settings.lang
    }

    fn log(&mut self, text: impl Into<String>) {
        self.output.push(text.into());
        if self.output.len() > 800 {
            self.output.drain(0..200);
        }
    }

    fn set_status(&mut self, text: impl Into<String>, kind: StatusKind) {
        self.status = Some(StatusMessage {
            text: text.into(),
            kind,
            at: Instant::now(),
        });
    }

    fn status_text(&self) -> Option<(String, Color32)> {
        let status = self.status.as_ref()?;
        if status.at.elapsed() > Duration::from_secs(6) {
            return None;
        }
        let color = match status.kind {
            StatusKind::Info => self.palette.fg_muted,
            StatusKind::Success => self.palette.success,
            StatusKind::Warning => self.palette.warning,
            StatusKind::Error => self.palette.error,
        };
        Some((status.text.clone(), color))
    }

    fn active_doc(&self) -> Option<&Document> {
        self.docs.get(self.active)
    }

    fn active_path(&self) -> Option<PathBuf> {
        self.active_doc().and_then(|doc| doc.path.clone())
    }

    fn workspace(&self) -> Option<PathBuf> {
        self.explorer.root.clone()
    }

    fn root_for_new_file(&self) -> PathBuf {
        self.active_path()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .or_else(|| self.workspace())
            .unwrap_or_else(home_dir)
    }

    fn overlay_open(&self, ctx: &egui::Context) -> bool {
        self.settings_open
            || self.about_open
            || self.help_open
            || self.quick.open
            || self.picker.open
            || self.native_picker.active()
            || self.prompt.is_some()
            || self.confirm.is_some()
            || ctx.memory(|m| m.any_popup_open())
    }

    fn apply_theme(&mut self, ctx: &egui::Context) {
        if (self.settings.font_size - self.applied_font_size).abs() > 0.01 {
            theme::install_fonts(
                ctx,
                self.settings.font_size,
                self.settings.font_size * 0.92,
            );
            self.applied_font_size = self.settings.font_size;
        }
        self.palette = theme::resolve(&self.settings);
        theme::apply_style(ctx, &self.settings, &self.palette);
        self.highlight_cache = None;
        self.settings_dirty = true;
    }

    /// Energy Saver trades the animated, AI-assisted experience for a quiet
    /// AMOLED editor: no animations, no ghost completions, true-black theme.
    fn toggle_energy_saver(&mut self, ctx: &egui::Context) {
        self.settings.energy_saver = !self.settings.energy_saver;
        if self.settings.energy_saver {
            self.settings.energy_backup = Some(EnergyBackup {
                animations: self.settings.animations,
                ghost: self.settings.ghost,
                theme: self.settings.theme.clone(),
            });
            self.settings.animations = false;
            self.settings.ghost = false;
            self.settings.theme = "AMOLED Black".to_owned();
            self.invalidate_ai();
            // Stop llama-server too: an idle model still holds hundreds of
            // megabytes on a modest machine.
            self.ai.suspend();
        } else if let Some(backup) = self.settings.energy_backup.take() {
            self.settings.animations = backup.animations;
            self.settings.ghost = backup.ghost;
            self.settings.theme = backup.theme;
        } else {
            self.settings.animations = true;
            self.settings.ghost = true;
        }
        if !self.settings.energy_saver {
            self.ai.restart(&self.settings);
        }
        self.apply_theme(ctx);
        ctx.request_repaint();
    }

    fn invalidate_editor(&mut self) {
        self.highlight_cache = None;
        self.spell_cache = None;
    }

    fn open_path(&mut self, path: PathBuf, line: Option<usize>, record_recent: bool) {
        let path = if path.is_absolute() {
            path
        } else {
            self.workspace()
                .unwrap_or_else(|| home_dir())
                .join(path)
        };
        if path.is_dir() {
            self.set_workspace(path, true);
            return;
        }
        if let Some(index) = self.docs.iter().position(|doc| doc.path.as_ref() == Some(&path)) {
            self.active = index;
            if let Some(line) = line {
                self.set_pending_line(index, line);
            }
            self.invalidate_ai();
            self.sync_preview_target(true);
            return;
        }
        match fs_util::read_text_file(&path) {
            Ok(text) => {
                let title = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("untitled")
                    .to_owned();
                let id = self.next_doc_id;
                self.next_doc_id += 1;
                let mut doc = Document {
                    id,
                    title,
                    path: Some(path.clone()),
                    text,
                    saved_text: String::new(),
                    language: Language::from_path(&path),
                    cursor_char: 0,
                    pending_cursor: None,
                    last_edit: Instant::now(),
                };
                doc.saved_text = doc.text.clone();
                if let Some(line) = line {
                    let cursor = line_start_char(&doc.text, line.saturating_sub(1));
                    doc.cursor_char = cursor;
                    doc.pending_cursor = Some(cursor);
                }
                self.docs.push(doc);
                self.active = self.docs.len() - 1;
                self.explorer.selected = Some(path.clone());
                if record_recent {
                    self.settings.push_recent_file(&path);
                    self.settings_dirty = true;
                }
                self.invalidate_ai();
                self.invalidate_editor();
                self.sync_preview_target(true);
                self.log(format!("Opened {}", path.display()));
            }
            Err(err) => {
                self.set_status(err.clone(), StatusKind::Error);
                self.log(err);
            }
        }
    }

    fn toggle_comment(&mut self) {
        let Some(doc) = self.docs.get_mut(self.active) else {
            return;
        };
        let (open, close) = doc.language.comment_affixes();
        if open.is_empty() {
            return;
        }
        let chars: Vec<char> = doc.text.chars().collect();
        let cursor = doc.cursor_char.min(chars.len());
        let start = chars[..cursor]
            .iter()
            .rposition(|c| *c == '\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        let end = chars[cursor..]
            .iter()
            .position(|c| *c == '\n')
            .map(|i| cursor + i)
            .unwrap_or(chars.len());
        let indent = chars[start..end]
            .iter()
            .take_while(|c| c.is_whitespace())
            .count();
        let indent_text: String = chars[start..start + indent].iter().collect();
        let content: String = chars[start + indent..end].iter().collect();
        let trailing = content.len() - content.trim_end_matches(char::is_whitespace).len();
        let core = &content[..content.len() - trailing];
        let trailing_text = &content[content.len() - trailing..];
        let open_trim = open.trim_end();
        let close_trim = close.trim_start();
        let commented = !core.is_empty()
            && core.starts_with(open_trim)
            && (close_trim.is_empty() || core.trim_end().ends_with(close_trim));
        let new_line = if commented {
            let mut body = core[open_trim.len()..].to_owned();
            if body.starts_with(' ') {
                body.remove(0);
            }
            if !close_trim.is_empty() {
                let trimmed = body.trim_end();
                if let Some(stripped) = trimmed.strip_suffix(close_trim) {
                    body = stripped.trim_end().to_owned();
                }
            }
            format!("{indent_text}{body}{trailing_text}")
        } else if core.is_empty() {
            format!("{indent_text}{open}{close}{trailing_text}")
        } else {
            format!("{indent_text}{open}{core}{close}{trailing_text}")
        };
        let byte_start = util::char_to_byte(&doc.text, start);
        let byte_end = util::char_to_byte(&doc.text, end);
        doc.text.replace_range(byte_start..byte_end, &new_line);
        let new_cursor = if commented {
            let space = usize::from(core[open_trim.len()..].starts_with(' '));
            cursor
                .saturating_sub(open_trim.chars().count() + space)
                .max(start + indent)
        } else {
            (cursor + open.chars().count()).min(start + new_line.chars().count())
        };
        doc.cursor_char = new_cursor;
        doc.pending_cursor = Some(new_cursor);
        doc.last_edit = Instant::now();
        self.completion.close();
        self.invalidate_editor();
        self.ai.suggestion = None;
        self.ai_due = None;
        self.schedule_ai();
    }

    fn set_pending_line(&mut self, index: usize, line: usize) {
        let Some(doc) = self.docs.get_mut(index) else {
            return;
        };
        let cursor = line_start_char(&doc.text, line.saturating_sub(1));
        doc.pending_cursor = Some(cursor);
        doc.cursor_char = cursor;
    }

    fn new_untitled(&mut self) {
        let id = self.next_doc_id;
        self.next_doc_id += 1;
        self.docs.push(Document {
            id,
            path: None,
            title: format!("Untitled-{id}"),
            text: String::new(),
            saved_text: String::new(),
            language: Language::Plain,
            cursor_char: 0,
            pending_cursor: None,
            last_edit: Instant::now(),
        });
        self.active = self.docs.len() - 1;
        self.invalidate_ai();
        self.invalidate_editor();
        self.set_status(t(self.lang(), "New file", "Novo arquivo"), StatusKind::Info);
    }

    fn close_all_docs(&mut self) {
        self.docs.clear();
        self.active = 0;
        self.pending_close_tab = None;
        self.completion.close();
        self.invalidate_ai();
        self.invalidate_editor();
        self.sync_preview_target(true);
        self.set_status(
            t(self.lang(), "All files closed", "Todos os arquivos fechados"),
            StatusKind::Success,
        );
    }

    fn request_close_project(&mut self) {
        if self.workspace().is_none() && self.docs.is_empty() {
            self.set_status(
                t(self.lang(), "No project is open", "Nenhum projeto aberto"),
                StatusKind::Info,
            );
            return;
        }
        if self.docs.iter().any(Document::modified) {
            self.confirm = Some(ConfirmDialog {
                kind: ConfirmKind::CloseProject,
                message: t(
                    self.lang(),
                    "You have unsaved changes. Close the project and all files without saving?",
                    "Você tem alterações não salvas. Fechar o projeto e todos os arquivos sem salvar?",
                )
                .to_owned(),
            });
        } else {
            self.close_project();
        }
    }

    fn close_project(&mut self) {
        self.close_all_docs();
        self.explorer.set_root(None);
        self.settings.last_workspace = None;
        self.settings_dirty = true;
        self.terminal.set_cwd(home_dir());
        self.show_preview = false;
        self.preview.stop();
        self.preview_server = None;
        self.preview_url.clear();
        self.preview_url_input.clear();
        self.preview_error = None;
        self.preview_target_doc = None;
        self.git = GitState::default();
        self.set_status(
            t(self.lang(), "Project closed", "Projeto fechado"),
            StatusKind::Success,
        );
        self.log("Workspace closed.");
    }

    fn close_doc(&mut self, index: usize) {
        if index >= self.docs.len() {
            return;
        }
        self.docs.remove(index);
        if self.docs.is_empty() {
            self.active = 0;
        } else if self.active >= self.docs.len() {
            self.active = self.docs.len() - 1;
        } else if index < self.active {
            self.active = self.active.saturating_sub(1);
        }
        self.invalidate_ai();
        self.invalidate_editor();
        self.sync_preview_target(true);
        // Beginner-friendly cleanup: once the last tab is gone and the
        // explorer is hidden, there is no project left to navigate. Close it
        // too instead of leaving an empty workspace in the background.
        if self.docs.is_empty() && !self.show_sidebar && self.workspace().is_some() {
            self.close_project();
        }
    }

    fn request_close_tab(&mut self, index: usize) {
        let Some(doc) = self.docs.get(index) else {
            return;
        };
        if doc.modified() {
            let name = doc.title.clone();
            self.confirm = Some(ConfirmDialog {
                kind: ConfirmKind::CloseTab(index),
                message: if self.lang().is_pt() {
                    format!("Deseja salvar as alterações em “{name}”?")
                } else {
                    format!("Save the changes to “{name}”?")
                },
            });
        } else {
            self.close_doc(index);
        }
    }

    fn save_doc(&mut self, index: usize) {
        let Some(doc) = self.docs.get(index) else {
            return;
        };
        let Some(path) = doc.path.clone() else {
            let start = self.workspace().unwrap_or_else(home_dir);
            if let Some(doc) = self.docs.get(index) {
                self.picker.file_name = doc.title.clone();
            }
            self.picker.open(PickerMode::SaveFile, Some(start), self.workspace());
            return;
        };
        let text = doc.text.clone();
        match fs_util::write_text_file(&path, &text) {
            Ok(()) => {
                if let Some(doc) = self.docs.get_mut(index) {
                    doc.saved_text = text;
                    doc.last_edit = Instant::now();
                }
                self.explorer.refresh_dir(path.parent().unwrap_or(&path));
                self.set_status(
                    format!(
                        "{} {}",
                        t(self.lang(), "Saved", "Salvo"),
                        path.file_name().and_then(|n| n.to_str()).unwrap_or("")
                    ),
                    StatusKind::Success,
                );
                self.log(format!("Saved {}", path.display()));
            }
            Err(err) => {
                self.set_status(err.clone(), StatusKind::Error);
                self.log(err);
            }
        }
    }

    fn save_all(&mut self) {
        for index in 0..self.docs.len() {
            if self.docs[index].modified() && self.docs[index].path.is_some() {
                self.save_doc(index);
            }
        }
    }

    fn save_as_result(&mut self, path: PathBuf) {
        let Some(doc) = self.docs.get_mut(self.active) else {
            return;
        };
        if doc.text.trim().is_empty() {
            if let Some(sample) = templates::sample_for_path(&path) {
                doc.text = sample.to_owned();
            }
        }
        match fs_util::write_text_file(&path, &doc.text) {
            Ok(()) => {
                doc.path = Some(path.clone());
                doc.title = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("untitled")
                    .to_owned();
                doc.saved_text = doc.text.clone();
                doc.language = Language::from_path(&path);
                let title = doc.title.clone();
                self.settings.push_recent_file(&path);
                self.settings_dirty = true;
                self.explorer.refresh();
                self.explorer.selected = Some(path.clone());
                self.invalidate_editor();
                self.sync_preview_target(true);
                self.log(format!("Saved as {}", path.display()));
                self.set_status(
                    format!("{} {}", t(self.lang(), "Saved", "Salvo"), title),
                    StatusKind::Success,
                );
            }
            Err(err) => {
                self.set_status(err.clone(), StatusKind::Error);
                self.log(err);
            }
        }
    }

    fn set_workspace(&mut self, path: PathBuf, persist: bool) {
        if !path.is_dir() {
            self.set_status(
                format!("{} is not a folder.", path.display()),
                StatusKind::Error,
            );
            return;
        }
        self.explorer.set_root(Some(path.clone()));
        self.explorer.set_show_hidden(self.settings.show_hidden);
        self.terminal.set_cwd(path.clone());
        self.highlight_cache = None;
        self.git = GitState::default();
        self.log(format!("Workspace opened: {}", path.display()));
        if persist {
            self.settings.last_workspace = Some(path.to_string_lossy().to_string());
            self.settings.push_recent_folder(&path);
            self.settings_dirty = true;
        }
        if let Some(server) = &self.preview_server {
            server.set_root(path.clone());
        }
        self.sync_preview_target(true);
        if self.show_preview {
            self.ensure_preview();
        }
    }

    fn ensure_preview(&mut self) {
        if self.preview_server.is_some() || self.preview_error.is_some() {
            return;
        }
        let root = self
            .workspace()
            .or_else(|| self.active_path().and_then(|p| p.parent().map(|p| p.to_path_buf())))
            .unwrap_or_else(|| welcome::ensure_workspace());
        match PreviewServer::start(self.settings.preview_port, root) {
            Ok(server) => {
                let url = server.url();
                self.log(format!("Preview server listening at {url}"));
                self.preview_server = Some(server);
                self.preview_error = None;
            }
            Err(err) => {
                self.log(err.clone());
                self.preview_error = Some(err);
            }
        }
    }

    fn preview_base_url(&self) -> String {
        self.preview_server
            .as_ref()
            .map(|s| s.url())
            .unwrap_or_else(|| format!("http://127.0.0.1:{}/", self.settings.preview_port))
    }

    fn auto_preview_url(&self) -> String {
        let base = self.preview_base_url();
        let Some(path) = self.active_path() else {
            return base;
        };
        let Some(root) = self.workspace() else {
            return base;
        };
        let Ok(relative) = path.strip_prefix(&root) else {
            return base;
        };
        let is_html = matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("html") | Some("htm")
        );
        if !is_html {
            return base;
        }
        format!("{base}{}", encode_path(relative))
    }

    fn sync_preview_target(&mut self, force: bool) {
        let doc_id = self.active_doc().map(|d| d.id);
        if !force && doc_id == self.preview_target_doc {
            return;
        }
        self.preview_target_doc = doc_id;
        let url = self.auto_preview_url();
        if url != self.preview_url {
            self.preview_url = url.clone();
            self.preview_url_input = url;
            self.preview.reload();
        } else if self.preview_url_input.is_empty() {
            self.preview_url_input = url;
        }
    }

    fn invalidate_ai(&mut self) {
        self.ai.invalidate();
        self.ai_due = None;
        self.ghost_controls_rect = None;
    }

    fn schedule_ai(&mut self) {
        if !self.settings.ai_enabled || !self.settings.ghost {
            return;
        }
        self.ai.invalidate();
        self.ghost_controls_rect = None;
        self.ai_due = Some(Instant::now() + Duration::from_millis(self.settings.completion_delay_ms));
    }

    fn tick_ai(&mut self, ctx: &egui::Context) {
        if self.ai.status.is_busy() {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
        let was_starting = self.ai.status == AiStatus::StartingServer;
        self.ai.tick();
        if was_starting && self.ai.status == AiStatus::Ready {
            // The server just finished loading; offer a suggestion even if the
            // user is already waiting on a line.
            self.schedule_ai();
        }
        if let Some(suggestion) = &self.ai.suggestion {
            let doc = self.active_doc();
            if doc.map(|d| d.cursor_char) != Some(suggestion.start) {
                // The cursor moved while Qwen was thinking. Ask again for the
                // new position instead of silently dropping the suggestion.
                self.ai.suggestion = None;
                self.ghost_controls_rect = None;
                self.schedule_ai();
            } else {
                self.completion.close();
            }
        }
    }

    fn maybe_request_ai(&mut self) {
        let Some(due) = self.ai_due else {
            return;
        };
        if Instant::now() < due {
            return;
        }
        if !self.settings.ai_enabled || !self.settings.ghost {
            self.ai_due = None;
            return;
        }
        match self.ai.status {
            AiStatus::Ready => {}
            AiStatus::StartingServer | AiStatus::Thinking => {
                // Keep the pending request alive until llama-server is up.
                self.ai_due = Some(Instant::now() + Duration::from_millis(450));
                return;
            }
            AiStatus::Off | AiStatus::Unavailable(_) => {
                self.ai_due = None;
                return;
            }
        }
        self.ai_due = None;
        let Some(doc) = self.active_doc() else {
            return;
        };
        if doc.text.is_empty() {
            return;
        }
        let text = doc.text.clone();
        let language = doc.language;
        let cursor = doc.cursor_char.min(text.chars().count());
        self.ai.request(&text, cursor, language, &self.settings);
    }

    fn open_settings(&mut self) {
        self.settings_snapshot = Some(self.settings.clone());
        self.settings_open = true;
        self.quick.open = false;
        self.prompt = None;
    }

    fn open_quick(&mut self, files: bool) {
        self.quick.open = true;
        self.quick.files_mode = files;
        self.quick.query.clear();
        self.quick.selected = 0;
        self.quick.just_opened = true;
        if files {
            let mut files = self
                .workspace()
                .map(|root| fs_util::walk_files(&root, 6000))
                .unwrap_or_default();
            files.sort();
            self.quick.files = files;
        }
    }

    fn refresh_git(&mut self) {
        let Some(root) = self.workspace() else {
            self.git = GitState::default();
            return;
        };
        if self.git.loaded_for.as_ref() == Some(&root) {
            return;
        }
        self.git.loaded_for = Some(root.clone());
        self.git.error = None;
        self.git.changes.clear();
        let branch = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output();
        match branch {
            Ok(output) if output.status.success() => {
                self.git.branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            }
            _ => {
                self.git.branch = String::new();
                self.git.error = Some(
                    t(
                        self.lang(),
                        "This folder is not a Git repository yet.",
                        "Esta pasta ainda não é um repositório Git.",
                    )
                    .to_owned(),
                );
                return;
            }
        }
        if let Ok(output) = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["status", "--porcelain"])
            .output()
        {
            for line in String::from_utf8_lossy(&output.stdout).lines().take(200) {
                if line.len() > 3 {
                    self.git
                        .changes
                        .push((line[..2].to_owned(), line[3..].to_owned()));
                }
            }
        }
    }

    fn start_search(&mut self) {
        let Some(root) = self.workspace() else {
            return;
        };
        let query = self.search.query.trim().to_owned();
        if query.is_empty() {
            self.search.hits.clear();
            self.search.rx = None;
            self.search.searching = false;
            return;
        }
        if query == self.search.last_started && self.search.rx.is_some() {
            return;
        }
        self.search.last_started = query.clone();
        self.search.searching = true;
        let case_sensitive = self.search.case_sensitive;
        let whole_word = self.search.whole_word;
        let (tx, rx) = channel();
        self.search.rx = Some(rx);
        std::thread::Builder::new()
            .name("kodemo-search".to_owned())
            .spawn(move || {
                let hits = search_workspace(&root, &query, case_sensitive, whole_word);
                let _ = tx.send(hits);
            })
            .ok();
    }

    fn tick_search(&mut self) {
        let mut received = None;
        if let Some(rx) = &self.search.rx {
            if let Ok(hits) = rx.try_recv() {
                received = Some(hits);
            }
        }
        if let Some(hits) = received {
            self.search.hits = hits;
            self.search.searching = false;
            self.search.rx = None;
        }
    }

    fn handle_explorer_intent(&mut self, intent: ExplorerIntent) {
        match intent {
            ExplorerIntent::Action(action) => self.handle_explorer_action(action),
        }
    }

    fn handle_explorer_action(&mut self, action: ExplorerAction) {
        match action {
            ExplorerAction::OpenFile(path) => self.open_path(path, None, true),
            ExplorerAction::NewFile(parent) => {
                self.prompt = Some(PromptDialog {
                    kind: PromptKind::NewFile(parent),
                    title: t(self.lang(), "New File", "Novo arquivo").to_owned(),
                    value: String::new(),
                    error: None,
                    just_opened: true,
                });
            }
            ExplorerAction::NewFolder(parent) => {
                self.prompt = Some(PromptDialog {
                    kind: PromptKind::NewFolder(parent),
                    title: t(self.lang(), "New Folder", "Nova pasta").to_owned(),
                    value: String::new(),
                    error: None,
                    just_opened: true,
                });
            }
            ExplorerAction::Rename(path) => {
                let value = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_owned();
                self.prompt = Some(PromptDialog {
                    kind: PromptKind::Rename(path),
                    title: t(self.lang(), "Rename", "Renomear").to_owned(),
                    value,
                    error: None,
                    just_opened: true,
                });
            }
            ExplorerAction::Delete(path) => {
                self.confirm = Some(ConfirmDialog {
                    kind: ConfirmKind::Delete(path.clone()),
                    message: format!(
                        "{} “{}”?",
                        t(self.lang(), "Delete", "Excluir"),
                        path.file_name().and_then(|n| n.to_str()).unwrap_or("")
                    ),
                });
                self.show_panel = true;
            }
            ExplorerAction::CopyPath(path) => {
                self.pending_command = Some(Command::CopyPath);
                self.open_path_for_copy(&path);
            }
            ExplorerAction::Reveal(path) => fs_util::reveal_in_file_manager(&path),
            ExplorerAction::ToggleHidden => {
                self.settings.show_hidden = !self.settings.show_hidden;
                self.explorer.set_show_hidden(self.settings.show_hidden);
                self.settings_dirty = true;
            }
            ExplorerAction::Refresh => {
                self.explorer.refresh();
                self.set_status(t(self.lang(), "Explorer refreshed", "Explorador atualizado"), StatusKind::Info);
            }
            ExplorerAction::OpenFolder => self.open_native_picker(PickerMode::OpenFolder),
        }
    }

    fn open_path_for_copy(&mut self, path: &Path) {
        // Copying happens through the command button; this keeps the path in the
        // status message and inside the context menu flow.
        self.set_status(
            format!("{} {}", t(self.lang(), "Path:", "Caminho:"), path.display()),
            StatusKind::Info,
        );
    }

    /// Save the open documents, then run the project the way its language
    /// expects: a compiler + binary in the terminal, or the live preview for
    /// static web pages.
    fn run_project(&mut self) {
        self.save_all();
        let active = self.active_path();
        let root = self
            .workspace()
            .or_else(|| active.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)))
            .unwrap_or_else(home_dir);
        match run::plan(&root, active.as_deref()) {
            Some(RunAction::Preview) => {
                self.show_preview = true;
                self.preview_error = None;
                self.ensure_preview();
                self.sync_preview_target(true);
                self.preview.reload();
                self.set_status(
                    t(
                        self.lang(),
                        "Preview started — no compile step for this page",
                        "Prévia iniciada — esta página não precisa de compilação",
                    ),
                    StatusKind::Success,
                );
                self.log(format!("Run: live preview in {}", root.display()));
            }
            Some(RunAction::Command(command)) => {
                self.show_panel = true;
                self.panel_tab = PanelTab::Terminal;
                self.terminal.set_cwd(root.clone());
                self.terminal.run_command(command.clone());
                self.set_status(
                    format!("{} {command}", t(self.lang(), "Running", "Executando")),
                    StatusKind::Info,
                );
                self.log(format!("Run: {command}"));
            }
            None => {
                self.set_status(
                    t(
                        self.lang(),
                        "No run task for this file. Open source code or a project folder.",
                        "Nenhuma tarefa de execução para este arquivo. Abra um código-fonte ou uma pasta de projeto.",
                    ),
                    StatusKind::Warning,
                );
            }
        }
    }

    /// Save and syntax-check/build the active language without running it.
    fn compile_project(&mut self) {
        self.save_all();
        let active = self.active_path();
        let root = self
            .workspace()
            .or_else(|| active.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)))
            .unwrap_or_else(home_dir);
        match run::compile_plan(&root, active.as_deref()) {
            Some(command) => {
                self.show_panel = true;
                self.panel_tab = PanelTab::Terminal;
                self.terminal.set_cwd(root.clone());
                self.terminal.run_command(command.clone());
                self.set_status(
                    format!("{} {command}", t(self.lang(), "Compiling", "Compilando")),
                    StatusKind::Info,
                );
                self.log(format!("Compile: {command}"));
            }
            None => self.set_status(
                t(
                    self.lang(),
                    "No compile task for this file.",
                    "Nenhuma tarefa de compilação para este arquivo.",
                ),
                StatusKind::Warning,
            ),
        }
    }

    fn execute_command(&mut self, command: Command, ctx: &egui::Context) {
        match command {
            Command::NewFile => {
                self.prompt = Some(PromptDialog {
                    kind: PromptKind::NewFile(self.root_for_new_file()),
                    title: t(self.lang(), "New File", "Novo arquivo").to_owned(),
                    value: String::new(),
                    error: None,
                    just_opened: true,
                });
            }
            Command::OpenFile => self.open_native_picker(PickerMode::OpenFile),
            Command::OpenFolder => self.open_native_picker(PickerMode::OpenFolder),
            Command::Save => self.save_doc(self.active),
            Command::SaveAs => {
                let start = self
                    .active_path()
                    .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                    .or_else(|| self.workspace())
                    .unwrap_or_else(home_dir);
                if let Some(doc) = self.active_doc() {
                    self.picker.file_name = doc.title.clone();
                }
                self.picker.open(PickerMode::SaveFile, Some(start), self.workspace());
            }
            Command::SaveAll => self.save_all(),
            Command::CloseTab => self.pending_close_tab = Some(self.active),
            Command::CloseAll => self.request_close_project(),
            Command::CloseProject => self.request_close_project(),
            Command::CloseOthers => {
                let active = self.active;
                if let Some(doc) = self.docs.get(active) {
                    let path = doc.path.clone();
                    let keep_id = doc.id;
                    let _ = path;
                    let indices: Vec<usize> = self
                        .docs
                        .iter()
                        .enumerate()
                        .filter(|(_, doc)| doc.id != keep_id && !doc.modified())
                        .map(|(i, _)| i)
                        .rev()
                        .collect();
                    for index in indices {
                        self.close_doc(index);
                    }
                    self.active = self.docs.iter().position(|d| d.id == keep_id).unwrap_or(0);
                }
            }
            Command::OpenSettings => self.open_settings(),
            Command::OpenAbout => self.about_open = true,
            Command::OpenHelp => self.help_open = true,
            Command::QuickOpen => self.open_quick(true),
            Command::CommandPalette => self.open_quick(false),
            Command::FindInFiles => {
                self.activity = Activity::Search;
                self.show_sidebar = true;
                self.search.focus = true;
            }
            Command::GoToLine => {
                self.prompt = Some(PromptDialog {
                    kind: PromptKind::GoToLine,
                    title: t(self.lang(), "Go to Line", "Ir para a linha").to_owned(),
                    value: String::new(),
                    error: None,
                    just_opened: true,
                });
            }
            Command::ToggleSidebar => self.show_sidebar = !self.show_sidebar,
            Command::TogglePanel => self.show_panel = !self.show_panel,
            Command::TogglePreview => {
                self.show_preview = !self.show_preview;
                if self.show_preview {
                    self.preview_error = None;
                    self.ensure_preview();
                    self.sync_preview_target(true);
                } else {
                    self.preview.stop();
                }
            }
            Command::ToggleTerminal => {
                if self.show_panel && self.panel_tab == PanelTab::Terminal {
                    self.show_panel = false;
                } else {
                    self.show_panel = true;
                    self.panel_tab = PanelTab::Terminal;
                }
            }
            Command::ToggleComment => self.toggle_comment(),
            Command::ToggleWordWrap => {
                self.settings.word_wrap = !self.settings.word_wrap;
                self.invalidate_editor();
                self.settings_dirty = true;
            }
            Command::ToggleMinimap => {
                self.settings.minimap = !self.settings.minimap;
                self.settings_dirty = true;
            }
            Command::ToggleGhost => {
                self.settings.ghost = !self.settings.ghost;
                self.invalidate_ai();
                self.settings_dirty = true;
            }
            Command::ToggleAi => {
                self.settings.ai_enabled = !self.settings.ai_enabled;
                self.ai.configure(&self.settings);
                self.settings_dirty = true;
                let label = if self.settings.ai_enabled {
                    t(self.lang(), "AI enabled", "IA ativada")
                } else {
                    t(self.lang(), "AI disabled", "IA desativada")
                };
                self.set_status(label, StatusKind::Info);
            }
            Command::ToggleEnergySaver => self.toggle_energy_saver(ctx),
            Command::StartPreview | Command::ReloadPreview => {
                self.show_preview = true;
                self.preview_error = None;
                self.ensure_preview();
                self.sync_preview_target(true);
                self.preview.reload();
            }
            Command::RunProject => self.run_project(),
            Command::CompileProject => self.compile_project(),
            Command::OpenInBrowser => {
                let url = if self.preview_url_input.trim().is_empty() {
                    self.preview_url.clone()
                } else {
                    self.preview_url_input.clone()
                };
                fs_util::open_external(&url);
            }
            Command::RestartAi => {
                self.ai.restart(&self.settings);
            }
            Command::ZoomIn => {
                self.settings.font_size = (self.settings.font_size + 1.0).min(30.0);
                self.apply_theme(ctx);
            }
            Command::ZoomOut => {
                self.settings.font_size = (self.settings.font_size - 1.0).max(10.0);
                self.apply_theme(ctx);
            }
            Command::ZoomReset => {
                self.settings.font_size = 15.0;
                self.apply_theme(ctx);
            }
            Command::ToggleFullscreen => {
                self.settings.fullscreen = !self.settings.fullscreen;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.settings.fullscreen));
                self.settings_dirty = true;
            }
            Command::NewTerminal => {
                self.show_panel = true;
                self.panel_tab = PanelTab::Terminal;
                self.terminal.clear();
                self.terminal.info(
                    t(
                        self.lang(),
                        "New terminal session. Commands run in the workspace.",
                        "Nova sessão de terminal. Os comandos rodam no workspace.",
                    )
                    .to_owned(),
                );
            }
            Command::ClearTerminal => self.terminal.clear(),
            Command::CopyPath => {
                if let Some(path) = self.active_path() {
                    ctx.copy_text(path.display().to_string());
                    self.set_status(
                        t(self.lang(), "Path copied", "Caminho copiado"),
                        StatusKind::Success,
                    );
                }
            }
            Command::RevealActive => {
                if let Some(path) = self.active_path() {
                    fs_util::reveal_in_file_manager(&path);
                }
            }
            Command::OpenRecentFile(value) => {
                let path = PathBuf::from(&value);
                if path.is_file() {
                    self.open_path(path, None, true);
                } else {
                    self.settings.recent_files.retain(|item| item != &value);
                    self.settings_dirty = true;
                    self.set_status(
                        t(
                            self.lang(),
                            "That recent file no longer exists.",
                            "Esse arquivo recente não existe mais.",
                        ),
                        StatusKind::Warning,
                    );
                }
            }
            Command::OpenRecentFolder(value) => {
                let path = PathBuf::from(&value);
                if path.is_dir() {
                    self.set_workspace(path, true);
                } else {
                    self.settings.recent_folders.retain(|item| item != &value);
                    self.settings_dirty = true;
                    self.set_status(
                        t(
                            self.lang(),
                            "That recent folder no longer exists.",
                            "Essa pasta recente não existe mais.",
                        ),
                        StatusKind::Warning,
                    );
                }
            }
            Command::ForgetRecent(value, is_folder) => {
                if is_folder {
                    self.settings.recent_folders.retain(|item| item != &value);
                } else {
                    self.settings.recent_files.retain(|item| item != &value);
                }
                self.settings_dirty = true;
            }
            Command::ClearRecent => {
                self.settings.recent_files.clear();
                self.settings.recent_folders.clear();
                self.settings_dirty = true;
            }
            Command::SetActivity(activity) => {
                self.activity = activity;
                self.show_sidebar = true;
                if activity == Activity::Git {
                    self.refresh_git();
                }
                if activity == Activity::Search {
                    self.search.focus = true;
                }
            }
            Command::Theme(name) => {
                self.settings.theme = name;
                if self.settings.theme == "Custom" && self.settings.custom_theme.is_none() {
                    self.settings.custom_theme = Some(self.palette.clone());
                }
                self.apply_theme(ctx);
                self.settings_dirty = true;
            }
            Command::Exit => self.request_exit(ctx),
        }
    }

    fn request_exit(&mut self, ctx: &egui::Context) {
        if self.docs.iter().any(Document::modified) {
            self.confirm = Some(ConfirmDialog {
                kind: ConfirmKind::CloseApp,
                message: t(
                    self.lang(),
                    "You have unsaved changes. Quit without saving?",
                    "Você tem alterações não salvas. Sair sem salvar?",
                )
                .to_owned(),
            });
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        } else {
            self.allow_close = true;
            let _ = self.settings.save();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn process_pending(&mut self, ctx: &egui::Context) {
        for _ in 0..6 {
            let mut did = false;
            if let Some((path, line)) = self.pending_open.take() {
                self.open_path(path, line, true);
                did = true;
            }
            if let Some(intent) = self.pending_explorer.take() {
                self.handle_explorer_intent(intent);
                did = true;
            }
            if let Some(index) = self.pending_close_tab.take() {
                self.request_close_tab(index);
                did = true;
            }
            if let Some(command) = self.pending_command.take() {
                self.execute_command(command, ctx);
                did = true;
            }
            if !did {
                break;
            }
        }
    }

    fn process_picker_result(&mut self, path: PathBuf) {
        match self.picker.mode {
            PickerMode::OpenFile => self.open_file_result(path),
            PickerMode::OpenFolder => self.set_workspace(path, true),
            PickerMode::SaveFile => self.save_as_result(path),
        }
    }

    fn open_file_result(&mut self, path: PathBuf) {
        self.open_path(path, None, true);
        if self.workspace().is_none() {
            if let Some(parent) = self
                .docs
                .get(self.active)
                .and_then(|d| d.path.as_ref())
                .and_then(|p| p.parent())
            {
                let parent = parent.to_path_buf();
                self.set_workspace(parent, true);
            }
        }
    }

    /// Use the native GTK file chooser when the desktop provides one. The
    /// built-in browser is still available as a dependency-free fallback.
    fn open_native_picker(&mut self, mode: PickerMode) {
        if self.native_picker.active() {
            return;
        }
        let start = match mode {
            PickerMode::OpenFile => self
                .active_path()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .or_else(|| self.workspace()),
            PickerMode::OpenFolder => self.workspace(),
            PickerMode::SaveFile => None,
        };
        let title = match mode {
            PickerMode::OpenFile => t(self.lang(), "Open File", "Abrir arquivo"),
            PickerMode::OpenFolder => t(self.lang(), "Open Folder", "Abrir pasta"),
            PickerMode::SaveFile => return,
        };
        if let Some(native_mode) = NativePickMode::from_picker(mode) {
            if self
                .native_picker
                .open(native_mode, start.clone(), title.to_owned())
            {
                return;
            }
        }
        self.picker.open(mode, start, self.workspace());
    }

    fn poll_native_picker(&mut self, ctx: &egui::Context) {
        if let Some((mode, picked)) = self.native_picker.poll() {
            if let Some(path) = picked {
                match mode {
                    NativePickMode::OpenFile => self.open_file_result(path),
                    NativePickMode::OpenFolder => self.set_workspace(path, true),
                }
            }
            ctx.request_repaint();
        }
        if self.native_picker.active() {
            ctx.request_repaint_after(Duration::from_millis(80));
        }
    }

    fn submit_prompt(&mut self, prompt: PromptDialog) {
        match prompt.kind.clone() {
            PromptKind::NewFile(parent) => {
                let Some(name) = util::sanitize_file_name(&prompt.value) else {
                    self.prompt = Some(PromptDialog {
                        error: Some(
                            t(self.lang(), "Type a valid file name.", "Digite um nome de arquivo válido.")
                                .to_owned(),
                        ),
                        ..prompt
                    });
                    return;
                };
                let path = parent.join(&name);
                if path.exists() {
                    self.prompt = Some(PromptDialog {
                        error: Some(t(self.lang(), "That file already exists.", "Esse arquivo já existe.").to_owned()),
                        ..prompt
                    });
                    return;
                }
                match fs_util::create_file(&path) {
                    Ok(()) => {
                        // Beginners get a small, runnable example instead of a
                        // blank buffer that gives no clue where to start.
                        if let Some(sample) = templates::sample_for_path(&path) {
                            let _ = fs::write(&path, sample);
                        }
                        if let Some(dir) = path.parent() {
                            self.explorer.refresh_dir(dir);
                            if let Some(parent) = dir.parent() {
                                self.explorer.expanded.insert(dir.to_path_buf());
                                let _ = parent;
                            }
                        }
                        self.open_path(path, None, true);
                    }
                    Err(err) => {
                        self.prompt = Some(PromptDialog {
                            error: Some(err),
                            ..prompt
                        });
                    }
                }
            }
            PromptKind::NewFolder(parent) => {
                let Some(name) = util::sanitize_file_name(&prompt.value) else {
                    self.prompt = Some(PromptDialog {
                        error: Some(t(self.lang(), "Type a valid folder name.", "Digite um nome de pasta válido.").to_owned()),
                        ..prompt
                    });
                    return;
                };
                let path = parent.join(&name);
                match fs_util::create_folder(&path) {
                    Ok(()) => {
                        self.explorer.refresh_dir(&parent);
                        self.set_status(
                            format!("{} {}", t(self.lang(), "Created", "Criado"), name),
                            StatusKind::Success,
                        );
                    }
                    Err(err) => {
                        self.prompt = Some(PromptDialog {
                            error: Some(err),
                            ..prompt
                        });
                    }
                }
            }
            PromptKind::Rename(old) => {
                let Some(name) = util::sanitize_file_name(&prompt.value) else {
                    self.prompt = Some(PromptDialog {
                        error: Some(t(self.lang(), "Type a valid name.", "Digite um nome válido.").to_owned()),
                        ..prompt
                    });
                    return;
                };
                let Some(parent) = old.parent() else {
                    return;
                };
                let new = parent.join(&name);
                if new.exists() && new != old {
                    self.prompt = Some(PromptDialog {
                        error: Some(t(self.lang(), "That name is already in use.", "Esse nome já está em uso.").to_owned()),
                        ..prompt
                    });
                    return;
                }
                match fs::rename(&old, &new) {
                    Ok(()) => {
                        for doc in &mut self.docs {
                            if doc.path.as_ref() == Some(&old) {
                                doc.path = Some(new.clone());
                                doc.title = name.clone();
                                doc.language = Language::from_path(&new);
                            } else if let Some(doc_path) = &doc.path {
                                if let Ok(rest) = doc_path.strip_prefix(&old) {
                                    doc.path = Some(new.join(rest));
                                }
                            }
                        }
                        self.explorer.refresh_dir(parent);
                        self.explorer.selected = Some(new);
                        self.invalidate_editor();
                        self.sync_preview_target(true);
                    }
                    Err(err) => {
                        self.prompt = Some(PromptDialog {
                            error: Some(format!("{err}")),
                            ..prompt
                        });
                    }
                }
            }
            PromptKind::GoToLine => {
                if let Ok(line) = prompt.value.trim().parse::<usize>() {
                    self.set_pending_line(self.active, line);
                } else {
                    self.prompt = Some(PromptDialog {
                        error: Some(t(self.lang(), "Type a line number.", "Digite um número de linha.").to_owned()),
                        ..prompt
                    });
                }
            }
        }
    }

    fn confirm_action(&mut self, kind: ConfirmKind, ctx: &egui::Context) {
        match kind {
            ConfirmKind::CloseTab(index) => self.close_doc(index),
            ConfirmKind::Delete(path) => {
                let result = if path.is_dir() {
                    fs::remove_dir_all(&path)
                } else {
                    fs::remove_file(&path)
                };
                match result {
                    Ok(()) => {
                        let mut index = 0;
                        while index < self.docs.len() {
                            let remove = self.docs[index]
                                .path
                                .as_ref()
                                .map(|p| p == &path || p.starts_with(&path))
                                .unwrap_or(false);
                            if remove {
                                self.docs.remove(index);
                            } else {
                                index += 1;
                            }
                        }
                        if self.active >= self.docs.len() {
                            self.active = self.docs.len().saturating_sub(1);
                        }
                        if let Some(parent) = path.parent() {
                            self.explorer.refresh_dir(parent);
                        }
                        self.set_status(
                            t(self.lang(), "Deleted", "Excluído"),
                            StatusKind::Success,
                        );
                        self.invalidate_editor();
                    }
                    Err(err) => {
                        self.set_status(format!("{err}"), StatusKind::Error);
                    }
                }
            }
            ConfirmKind::CloseProject => self.close_project(),
            ConfirmKind::CloseApp => {
                self.allow_close = true;
                let _ = self.settings.save();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

impl Drop for KodemoApp {
    fn drop(&mut self) {
        self.preview.stop();
        let _ = self.settings.save();
    }
}

impl eframe::App for KodemoApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette.bg.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if !self.close_requested_handled {
                self.close_requested_handled = true;
                if self.docs.iter().any(Document::modified) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    self.confirm = Some(ConfirmDialog {
                        kind: ConfirmKind::CloseApp,
                        message: t(
                            self.lang(),
                            "You have unsaved changes. Quit without saving?",
                            "Você tem alterações não salvas. Sair sem salvar?",
                        )
                        .to_owned(),
                    });
                } else {
                    self.allow_close = true;
                    let _ = self.settings.save();
                }
            }
        } else if !ctx.input(|i| i.viewport().close_requested()) {
            self.close_requested_handled = false;
        }

        if !self.view_initialized {
            self.view_initialized = true;
            if self.settings.fullscreen {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
            } else if self.settings.maximized {
                // Apply the maximize request after the window exists. Some
                // lightweight window managers ignore it in the initial map.
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
            }
        }

        self.proc_stats.sample();
        self.tick_ai(ctx);
        self.tick_search();
        self.poll_native_picker(ctx);
        self.terminal.poll();
        self.handle_shortcuts(ctx);

        self.show_menubar(ctx);
        self.show_statusbar(ctx);
        if self.show_panel {
            self.show_bottom_panel(ctx);
        }
        let activity_color = self.palette.activity;
        SidePanel::left("kodemo-activity-bar")
            .exact_width(46.0)
            .resizable(false)
            .frame(Frame::NONE.fill(activity_color))
            .show(ctx, |ui| self.show_activity_bar(ui));
        if self.show_sidebar || self.settings.animations {
            self.show_sidebar(ctx);
        }

        self.pending_preview_rect = None;
        if self.show_preview {
            self.show_preview_panel(ctx);
        }

        self.show_editor_area(ctx);
        self.process_pending(ctx);
        self.auto_save_docs();

        self.show_prompt(ctx);
        self.show_confirm(ctx);
        self.show_quick_open(ctx);
        let picked = self.picker.show(ctx, &self.palette, self.lang());
        if let Some(path) = picked {
            self.process_picker_result(path);
        }
        self.show_settings(ctx);
        self.show_about(ctx);
        self.show_help(ctx);
        self.show_completion(ctx);
        let hovered = self
            .picker
            .hovered
            .clone()
            .or_else(|| self.explorer.hovered.clone());
        self.hover_preview.show(
            ctx,
            hovered,
            self.settings.hover_previews,
            self.settings.hover_audio,
            self.lang(),
            &self.palette,
        );
        self.maybe_request_ai();
        self.process_pending(ctx);

        let overlay = self.overlay_open(ctx);
        let preview_url = self.preview_url.clone();
        let visible = self.show_preview && !overlay;
        let pixels_per_point = ctx.pixels_per_point();
        if self.show_preview {
            self.preview.update(
                frame,
                self.pending_preview_rect,
                visible,
                &preview_url,
                pixels_per_point,
                self.software,
            );
        } else {
            self.preview.update(frame, None, false, "", pixels_per_point, self.software);
        }

        let title = match self.active_doc() {
            Some(doc) => format!(
                "{}{} — Kodemo",
                if doc.modified() { "● " } else { "" },
                doc.title
            ),
            None => "Kodemo".to_owned(),
        };
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }

        if self.settings_dirty && self.last_settings_save.elapsed() > Duration::from_millis(900) {
            let _ = self.settings.save();
            self.settings_dirty = false;
            self.last_settings_save = Instant::now();
        }
        if self.settings_dirty {
            let wait = Duration::from_millis(900)
                .saturating_sub(self.last_settings_save.elapsed());
            ctx.request_repaint_after(wait.max(Duration::from_millis(50)));
        }

        if self.ai.status.is_busy() || self.search.searching {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if let Some(due) = self.ai_due {
            let wait = due.saturating_duration_since(Instant::now());
            ctx.request_repaint_after(wait.max(Duration::from_millis(30)));
        }
        if self.started.elapsed() < Duration::from_secs(2) {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
        // Refresh the CPU/RAM readout about once a second. Energy Saver skips
        // the wake-up so an idle window stays completely idle.
        if !self.settings.energy_saver {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
    }
}

impl KodemoApp {
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.quick.open
            || self.settings_open
            || self.picker.open
            || self.native_picker.active()
            || self.prompt.is_some()
            || self.confirm.is_some()
        {
            return;
        }
        if self.completion.open {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                self.completion.close();
                return;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                if !self.completion.items.is_empty() {
                    self.completion.selected =
                        (self.completion.selected + 1).min(self.completion.items.len() - 1);
                }
                ctx.request_repaint();
                return;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                self.completion.selected = self.completion.selected.saturating_sub(1);
                ctx.request_repaint();
                return;
            }
            if self.ai.suggestion.is_none()
                && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab))
            {
                self.accept_completion(self.completion.selected);
                return;
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                self.accept_completion(self.completion.selected);
                return;
            }
        }
        if self.ai.suggestion.is_some() {
            let editor_id = self.docs.get(self.active).map(|doc| editor_id(doc.id));
            let focused = ctx.memory(|m| m.focused());
            if editor_id == focused
                && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab))
            {
                self.accept_ghost();
                return;
            }
        }
        let shortcut = |modifiers: Modifiers, key: Key| {
            egui::KeyboardShortcut::new(modifiers, key)
        };
        let mut command = None;
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL | Modifiers::SHIFT, Key::P))) {
            command = Some(Command::CommandPalette);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::P))) {
            command = Some(Command::QuickOpen);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL | Modifiers::SHIFT, Key::S))) {
            command = Some(Command::SaveAs);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::S))) {
            command = Some(Command::Save);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL | Modifiers::SHIFT, Key::O))) {
            command = Some(Command::OpenFolder);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::O))) {
            command = Some(Command::OpenFile);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::N))) {
            command = Some(Command::NewFile);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::W))) {
            command = Some(Command::CloseTab);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::B))) {
            command = Some(Command::ToggleSidebar);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::J))) {
            command = Some(Command::TogglePanel);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL | Modifiers::ALT, Key::P))) {
            command = Some(Command::TogglePreview);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL | Modifiers::SHIFT, Key::B))) {
            command = Some(Command::CompileProject);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL | Modifiers::SHIFT, Key::F))) {
            command = Some(Command::FindInFiles);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::G))) {
            command = Some(Command::GoToLine);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::Slash))) {
            command = Some(Command::ToggleComment);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::Comma))) {
            command = Some(Command::OpenSettings);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::Backtick))) {
            command = Some(Command::ToggleTerminal);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL | Modifiers::SHIFT, Key::Backtick))) {
            command = Some(Command::NewTerminal);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::Equals)))
            || ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::Plus)))
        {
            command = Some(Command::ZoomIn);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::Minus))) {
            command = Some(Command::ZoomOut);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::Num0))) {
            command = Some(Command::ZoomReset);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL, Key::F5))) {
            command = Some(Command::RunProject);
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F5)) {
            command = Some(Command::ReloadPreview);
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F11)) {
            command = Some(Command::ToggleFullscreen);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::CTRL | Modifiers::SHIFT, Key::F))) {
            command = Some(Command::FindInFiles);
        }
        if let Some(command) = command {
            self.pending_command = Some(command);
        }
    }

    fn show_menubar(&mut self, ctx: &egui::Context) {
        let lang = self.lang();
        let palette = self.palette.clone();
        let recent_files = self.settings.recent_files.clone();
        let recent_folders = self.settings.recent_folders.clone();
        let theme_current = self.settings.theme.clone();
        let mut command: Option<Command> = None;
        let mut open_quick_files = false;
        let mut toggle_sidebar = false;
        let mut toggle_panel = false;
        let mut toggle_preview = false;
        let sidebar_active = self.show_sidebar;
        let panel_active = self.show_panel;
        let preview_active = self.show_preview;
        let ai_status = self.ai.status.clone();
        let ai_color = match &ai_status {
            AiStatus::Ready => palette.success,
            AiStatus::Thinking | AiStatus::StartingServer => palette.warning,
            AiStatus::Unavailable(_) => palette.error,
            AiStatus::Off => palette.fg_faint,
        };

        TopBottomPanel::top("kodemo-menubar")
            .exact_height(35.0)
            .frame(Frame::NONE.fill(palette.titlebar))
            .show(ctx, |ui| {
                ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                ui.visuals_mut().widgets.inactive.bg_fill = Color32::TRANSPARENT;
                ui.visuals_mut().widgets.hovered.weak_bg_fill = palette.hover;
                ui.visuals_mut().widgets.hovered.bg_fill = palette.hover;
                ui.visuals_mut().widgets.active.weak_bg_fill = palette.panel_alt;
                ui.visuals_mut().widgets.active.bg_fill = palette.panel_alt;
                ui.horizontal_centered(|ui| {
                    ui.add_space(6.0);
                    let (brand_rect, _) = ui.allocate_exact_size(
                        Vec2::new(88.0, 26.0),
                        Sense::hover(),
                    );
                    if ui.rect_contains_pointer(brand_rect) {
                        ui.painter()
                            .rect_filled(brand_rect, 6.0, palette.hover);
                    }
                    let logo_rect = egui::Rect::from_center_size(
                        Pos2::new(brand_rect.left() + 15.0, brand_rect.center().y),
                        Vec2::splat(15.0),
                    );
                    icons::draw_logo_mark(ui.painter(), logo_rect, palette.accent);
                    ui.painter().text(
                        Pos2::new(brand_rect.left() + 27.0, brand_rect.center().y),
                        Align2::LEFT_CENTER,
                        "KODEMO",
                        FontId::proportional(12.5),
                        palette.fg,
                    );
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(4.0);

                    ui.menu_button(t(lang, "File", "Arquivo"), |ui| {
                        ui.set_min_width(240.0);
                        menu_item(ui, t(lang, "New File…", "Novo arquivo…"), "Ctrl+N", Command::NewFile, &mut command);
                        menu_item(ui, t(lang, "Open File…", "Abrir arquivo…"), "Ctrl+O", Command::OpenFile, &mut command);
                        menu_item(ui, t(lang, "Open Folder…", "Abrir pasta…"), "Ctrl+Shift+O", Command::OpenFolder, &mut command);
                        ui.add_space(2.0);
                        ui.menu_button(t(lang, "Open Recent", "Abrir recente"), |ui| {
                            ui.set_min_width(300.0);
                            if recent_files.is_empty() && recent_folders.is_empty() {
                                ui.label(RichText::new(t(lang, "No recent items", "Nenhum item recente")).color(palette.fg_faint));
                            }
                            for file in recent_files.iter().take(10) {
                                ui.horizontal(|ui| {
                                    let label = util::trimmed_center(&format_relative(Path::new(file)), 58);
                                    if ui.add(egui::Button::new(label).frame(false).min_size(Vec2::new(240.0, 0.0))).clicked() {
                                        command = Some(Command::OpenRecentFile(file.clone()));
                                        ui.close_menu();
                                    }
                                    if icon_button(ui, Icon::Close, 16.0, &palette, t(lang, "Remove from recent files", "Remover dos arquivos recentes")).clicked() {
                                        command = Some(Command::ForgetRecent(file.clone(), false));
                                        ui.close_menu();
                                    }
                                });
                            }
                            if !recent_folders.is_empty() {
                                ui.separator();
                                for folder in recent_folders.iter().take(6) {
                                    ui.horizontal(|ui| {
                                        let label = util::trimmed_center(&format_relative(Path::new(folder)), 58);
                                        if ui.add(egui::Button::new(label).frame(false).min_size(Vec2::new(240.0, 0.0))).clicked() {
                                            command = Some(Command::OpenRecentFolder(folder.clone()));
                                            ui.close_menu();
                                        }
                                        if icon_button(ui, Icon::Close, 16.0, &palette, t(lang, "Remove from recent folders", "Remover das pastas recentes")).clicked() {
                                            command = Some(Command::ForgetRecent(folder.clone(), true));
                                            ui.close_menu();
                                        }
                                    });
                                }
                            }
                            if !recent_files.is_empty() || !recent_folders.is_empty() {
                                ui.separator();
                                menu_item(ui, t(lang, "Clear Recent", "Limpar recentes"), "", Command::ClearRecent, &mut command);
                            }
                        });
                        ui.separator();
                        menu_item(ui, t(lang, "Save", "Salvar"), "Ctrl+S", Command::Save, &mut command);
                        menu_item(ui, t(lang, "Save As…", "Salvar como…"), "Ctrl+Shift+S", Command::SaveAs, &mut command);
                        menu_item(ui, t(lang, "Save All", "Salvar tudo"), "", Command::SaveAll, &mut command);
                        ui.separator();
                        menu_item(ui, t(lang, "Close Editor", "Fechar editor"), "Ctrl+W", Command::CloseTab, &mut command);
                        menu_item(ui, t(lang, "Close Other Editors", "Fechar outros editores"), "", Command::CloseOthers, &mut command);
                        menu_item(ui, t(lang, "Close All Files and Project", "Fechar todos os arquivos e projeto"), "", Command::CloseAll, &mut command);
                        menu_item(ui, t(lang, "Close Project", "Fechar projeto"), "", Command::CloseProject, &mut command);
                        ui.separator();
                        menu_item(ui, t(lang, "Settings…", "Configurações…"), "Ctrl+,", Command::OpenSettings, &mut command);
                        menu_item(ui, t(lang, "Exit", "Sair"), "", Command::Exit, &mut command);
                    });

                    ui.menu_button(t(lang, "Edit", "Editar"), |ui| {
                        ui.set_min_width(240.0);
                        menu_item(ui, t(lang, "Command Palette…", "Paleta de comandos…"), "Ctrl+Shift+P", Command::CommandPalette, &mut command);
                        menu_item(ui, t(lang, "Quick Open…", "Abrir rápido…"), "Ctrl+P", Command::QuickOpen, &mut command);
                        ui.separator();
                        menu_item(ui, t(lang, "Find in Files…", "Localizar nos arquivos…"), "Ctrl+Shift+F", Command::FindInFiles, &mut command);
                        menu_item(ui, t(lang, "Go to Line…", "Ir para linha…"), "Ctrl+G", Command::GoToLine, &mut command);
                        ui.separator();
                        menu_item(ui, t(lang, "Toggle Line Comment", "Comentar linha"), "Ctrl+/", Command::ToggleComment, &mut command);
                        menu_item(ui, t(lang, "Copy Path", "Copiar caminho"), "", Command::CopyPath, &mut command);
                    });

                    ui.menu_button(t(lang, "View", "Exibir"), |ui| {
                        ui.set_min_width(240.0);
                        menu_toggle(ui, t(lang, "Sidebar", "Barra lateral"), sidebar_active, "Ctrl+B", Command::ToggleSidebar, &mut command);
                        menu_toggle(ui, t(lang, "Panel", "Painel"), panel_active, "Ctrl+J", Command::TogglePanel, &mut command);
                        menu_toggle(ui, t(lang, "Live Preview", "Prévia ao vivo"), preview_active, "Ctrl+Alt+P", Command::TogglePreview, &mut command);
                        ui.separator();
                        menu_item(ui, t(lang, "Word Wrap", "Quebra de linha"), "", Command::ToggleWordWrap, &mut command);
                        menu_item(ui, t(lang, "Ghost Suggestions", "Sugestões fantasma"), "", Command::ToggleGhost, &mut command);
                        ui.separator();
                        ui.menu_button(t(lang, "Theme", "Tema"), |ui| {
                            for name in theme::theme_names() {
                                let selected = theme_current == *name;
                                if ui
                                    .add(
                                        egui::Button::new(format!(
                                            "{} {}",
                                            if selected { "●" } else { "○" },
                                            theme_name(lang, name)
                                        ))
                                        .frame(false)
                                        .min_size(Vec2::new(190.0, 0.0)),
                                    )
                                    .clicked()
                                {
                                    command = Some(Command::Theme((*name).to_owned()));
                                    ui.close_menu();
                                }
                            }
                        });
                        ui.separator();
                        menu_item(ui, t(lang, "Zoom In", "Aumentar zoom"), "Ctrl+=", Command::ZoomIn, &mut command);
                        menu_item(ui, t(lang, "Zoom Out", "Diminuir zoom"), "Ctrl+-", Command::ZoomOut, &mut command);
                        menu_item(ui, t(lang, "Reset Zoom", "Redefinir zoom"), "Ctrl+0", Command::ZoomReset, &mut command);
                        menu_item(ui, t(lang, "Full Screen", "Tela cheia"), "F11", Command::ToggleFullscreen, &mut command);
                    });

                    ui.menu_button(t(lang, "Run", "Executar"), |ui| {
                        ui.set_min_width(240.0);
                        menu_item(ui, t(lang, "Run Project", "Executar projeto"), "Ctrl+F5", Command::RunProject, &mut command);
                        menu_item(ui, t(lang, "Compile Project", "Compilar projeto"), "Ctrl+Shift+B", Command::CompileProject, &mut command);
                        ui.separator();
                        menu_item(ui, t(lang, "Start Live Preview", "Iniciar prévia ao vivo"), "Ctrl+Alt+P", Command::StartPreview, &mut command);
                        menu_item(ui, t(lang, "Reload Preview", "Recarregar prévia"), "F5", Command::ReloadPreview, &mut command);
                        menu_item(ui, t(lang, "Open in Browser", "Abrir no navegador"), "", Command::OpenInBrowser, &mut command);
                        ui.separator();
                        menu_item(ui, t(lang, "Toggle AI", "Alternar IA"), "", Command::ToggleAi, &mut command);
                        menu_item(ui, t(lang, "Restart AI Server", "Reiniciar servidor de IA"), "", Command::RestartAi, &mut command);
                    });

                    ui.menu_button(t(lang, "Terminal", "Terminal"), |ui| {
                        ui.set_min_width(240.0);
                        menu_item(ui, t(lang, "Toggle Terminal", "Alternar terminal"), "Ctrl+`", Command::ToggleTerminal, &mut command);
                        menu_item(ui, t(lang, "New Terminal", "Novo terminal"), "Ctrl+Shift+`", Command::NewTerminal, &mut command);
                        menu_item(ui, t(lang, "Clear", "Limpar"), "", Command::ClearTerminal, &mut command);
                    });

                    ui.menu_button(t(lang, "Help", "Ajuda"), |ui| {
                        ui.set_min_width(240.0);
                        menu_item(ui, t(lang, "Welcome", "Boas-vindas"), "", Command::OpenAbout, &mut command);
                        menu_item(ui, t(lang, "Keyboard Shortcuts", "Atalhos de teclado"), "", Command::OpenHelp, &mut command);
                        menu_item(ui, t(lang, "About Kodemo", "Sobre o Kodemo"), "", Command::OpenAbout, &mut command);
                    });

                    ui.add_space(8.0);
                    let search_width = (ui.available_width() - 300.0).clamp(120.0, 280.0);
                    let (search_rect, search) = ui.allocate_exact_size(
                        Vec2::new(search_width, 22.0),
                        Sense::click(),
                    );
                    let search_fill = if search.hovered() {
                        palette.hover
                    } else {
                        palette.panel_alt
                    };
                    ui.painter().rect_filled(search_rect, 5.0, search_fill);
                    let glyph_rect = egui::Rect::from_center_size(
                        Pos2::new(search_rect.left() + 13.0, search_rect.center().y),
                        Vec2::splat(12.0),
                    );
                    icons::draw(
                        ui.painter(),
                        glyph_rect,
                        Icon::Search,
                        palette.fg_faint,
                        1.1,
                    );
                    ui.painter().text(
                        Pos2::new(search_rect.left() + 24.0, search_rect.center().y),
                        Align2::LEFT_CENTER,
                        t(lang, "Search files  Ctrl+P", "Buscar arquivos  Ctrl+P"),
                        FontId::proportional(11.5),
                        palette.fg_faint,
                    );
                    if search.clicked() {
                        open_quick_files = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if title_icon_button(ui, Icon::Settings, false, &palette, t(lang, "Settings", "Configurações")).clicked() {
                            command = Some(Command::OpenSettings);
                        }
                        if title_run_button(ui, &palette, t(lang, "Run", "Executar"), t(lang, "Run project  Ctrl+F5", "Executar projeto  Ctrl+F5")).clicked() {
                            command = Some(Command::RunProject);
                        }
                        if title_icon_button(ui, Icon::Preview, preview_active, &palette, t(lang, "Live Preview", "Prévia ao vivo")).clicked() {
                            toggle_preview = true;
                        }
                        if title_icon_button(ui, Icon::Panel, panel_active, &palette, t(lang, "Panel", "Painel")).clicked() {
                            toggle_panel = true;
                        }
                        if title_icon_button(ui, Icon::Sidebar, sidebar_active, &palette, t(lang, "Sidebar", "Barra lateral")).clicked() {
                            toggle_sidebar = true;
                        }
                        ui.add_space(4.0);
                        let ai_ready = matches!(ai_status, AiStatus::Ready);
                        let ai_text = if ai_ready {
                            t(lang, "AI", "IA")
                        } else if ai_status.is_busy() {
                            "…"
                        } else {
                            "AI"
                        };
                        let response = ui.label(
                            RichText::new(format!("● {ai_text}"))
                                .size(10.5)
                                .color(ai_color),
                        );
                        response.on_hover_text(ai_status.short());
                        ui.add_space(6.0);
                        if energy_saver_button(
                            ui,
                            self.settings.energy_saver,
                            &palette,
                            t(
                                lang,
                                "Energy Saver — fewer animations, no AI ghost text, AMOLED black",
                                "Economia de energia — menos animações, sem texto fantasma da IA, preto AMOLED",
                            ),
                        )
                        .clicked()
                        {
                            command = Some(Command::ToggleEnergySaver);
                        }
                        ui.add_space(4.0);
                        let stats = ui.label(
                            RichText::new(format!(
                                "CPU {:.0}%  ·  {:.0} MB",
                                self.proc_stats.cpu_percent, self.proc_stats.memory_mb
                            ))
                            .size(10.0)
                            .color(palette.fg_faint),
                        );
                        stats.on_hover_text(t(
                            lang,
                            "CPU and memory used by Kodemo, its webview and Qwen (summed from /proc)",
                            "CPU e memória usados pelo Kodemo, seu webview e o Qwen (somados de /proc)",
                        ));
                    });
                });
                let rect = ui.max_rect();
                ui.painter().hline(
                    rect.x_range(),
                    rect.bottom() - 0.5,
                    Stroke::new(1.0_f32, palette.border),
                );
            });

        if open_quick_files {
            self.open_quick(true);
        }
        if toggle_sidebar {
            self.show_sidebar = !self.show_sidebar;
        }
        if toggle_panel {
            self.show_panel = !self.show_panel;
        }
        if toggle_preview {
            self.pending_command = Some(Command::TogglePreview);
        }
        if let Some(command) = command {
            self.pending_command = Some(command);
        }
    }

    fn show_statusbar(&mut self, ctx: &egui::Context) {
        let lang = self.lang();
        let palette = self.palette.clone();
        let (line, col) = self
            .active_doc()
            .map(|doc| util::byte_line_col(&doc.text, util::char_to_byte(&doc.text, doc.cursor_char)))
            .unwrap_or((1, 1));
        let language = self
            .active_doc()
            .map(|doc| doc.language.display())
            .unwrap_or("—");
        let branch = if self.git.branch.is_empty() {
            "main".to_owned()
        } else {
            self.git.branch.clone()
        };
        let ai_status = self.ai.status.clone();
        let ai_color = match &ai_status {
            AiStatus::Ready => palette.success,
            AiStatus::Thinking | AiStatus::StartingServer => palette.warning,
            AiStatus::Unavailable(_) => palette.error,
            AiStatus::Off => palette.fg_faint,
        };
        let status_text = self.status_text();
        let ai_short = match &ai_status {
            AiStatus::Ready => t(lang, "AI ready", "IA pronta"),
            AiStatus::Thinking => t(lang, "Qwen thinking…", "Qwen pensando…"),
            AiStatus::StartingServer => t(lang, "Starting Qwen…", "Iniciando Qwen…"),
            AiStatus::Off => t(lang, "AI off", "IA desligada"),
            AiStatus::Unavailable(_) => t(lang, "AI unavailable", "IA indisponível"),
        };
        let ai_elapsed = self
            .ai
            .elapsed()
            .map(|elapsed| format!(" {:.1}s", elapsed.as_secs_f32()))
            .unwrap_or_default();
        let ai_label = format!("● {ai_short}{ai_elapsed}");
        let mut command = None;

        TopBottomPanel::bottom("kodemo-statusbar")
            .exact_height(23.0)
            .frame(Frame::NONE.fill(palette.statusbar))
            .show(ctx, |ui| {
                let fg = if palette.dark { Color32::from_gray(230) } else { Color32::from_rgb(245, 245, 245) };
                ui.horizontal_centered(|ui| {
                    ui.add_space(6.0);
                    inline_icon(ui, Icon::Branch, 15.0, fg);
                    let branch_response = ui.label(
                        RichText::new(branch).size(11.0).color(fg),
                    );
                    branch_response.on_hover_text(t(lang, "Current Git branch", "Branch Git atual"));
                    if inline_icon_button(ui, Icon::Refresh, 17.0, fg, t(lang, "Refresh source control", "Atualizar controle de código")).clicked() {
                        self.refresh_git();
                    }
                    ui.add_space(6.0);
                    if let Some((text, _)) = &status_text {
                        ui.label(RichText::new(text).size(11.0).color(fg));
                    } else {
                        inline_icon(ui, Icon::Error, 14.0, fg);
                        ui.label(RichText::new("0").size(11.0).color(fg));
                        ui.add_space(4.0);
                        inline_icon(ui, Icon::Warning, 14.0, fg);
                        ui.label(RichText::new("0").size(11.0).color(fg));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(6.0);
                        if ui.add(egui::Button::new(RichText::new(ai_label).size(10.5).color(ai_color)).frame(false)).on_hover_text(match &ai_status {
                            AiStatus::Unavailable(err) => err.as_str(),
                            _ => t(lang, "Local Qwen completions", "Conclusões locais do Qwen"),
                        }).clicked() {
                            command = Some(Command::SetActivity(Activity::Ai));
                        }
                        ui.add_space(8.0);
                        ui.label(RichText::new(format!("Ln {line}, Col {col}")).size(11.0).color(fg));
                        ui.add_space(8.0);
                        ui.label(RichText::new(language).size(11.0).color(fg));
                        ui.add_space(8.0);
                        ui.label(RichText::new("UTF-8").size(11.0).color(fg));
                        ui.add_space(8.0);
                        ui.label(RichText::new(format!("Spaces: {}", self.settings.tab_size)).size(11.0).color(fg));
                        ui.add_space(8.0);
                        if inline_icon_button(ui, Icon::Settings, 18.0, fg, t(lang, "Settings", "Configurações")).clicked() {
                            command = Some(Command::OpenSettings);
                        }
                    });
                });
            });
        if let Some(command) = command {
            self.pending_command = Some(command);
        }
    }
}

impl KodemoApp {
    fn show_activity_bar(&mut self, ui: &mut Ui) {
        let palette = self.palette.clone();
        let lang = self.lang();
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.add_space(5.0);
        let activities = [
            Activity::Explorer,
            Activity::Search,
            Activity::Git,
            Activity::Extensions,
            Activity::Ai,
        ];
        let mut selected = None;
        for activity in activities {
            let active = self.activity == activity && self.show_sidebar;
            let response = activity_button(
                ui,
                activity.icon(),
                active,
                &palette,
                activity.title(lang),
            );
            if response.clicked() {
                selected = Some(activity);
            }
        }
        ui.add_space((ui.available_height() - 92.0).max(0.0));
        if activity_button(
            ui,
            Icon::Settings,
            false,
            &palette,
            t(lang, "Settings", "Configurações"),
        )
        .clicked()
        {
            self.pending_command = Some(Command::OpenSettings);
        }
        if activity_button(
            ui,
            Icon::Account,
            false,
            &palette,
            t(lang, "About Kodemo", "Sobre o Kodemo"),
        )
        .clicked()
        {
            self.about_open = true;
        }
        if let Some(activity) = selected {
            if self.activity == activity && self.show_sidebar {
                self.show_sidebar = false;
            } else {
                self.activity = activity;
                self.show_sidebar = true;
                if activity == Activity::Git {
                    self.refresh_git();
                }
                if activity == Activity::Search {
                    self.search.focus = true;
                }
            }
        }
    }

    fn show_sidebar(&mut self, ctx: &egui::Context) {
        let palette = self.palette.clone();
        let animation_time = if self.settings.animations { 0.18 } else { 0.0 };
        let t = if animation_time > 0.0 {
            ctx.animate_bool_with_time_and_easing(
                Id::new("kodemo-sidebar-slide"),
                self.show_sidebar,
                animation_time,
                egui::emath::easing::cubic_out,
            )
        } else if self.show_sidebar {
            1.0
        } else {
            0.0
        };
        if t <= 0.001 {
            return;
        }
        let animating = t < 0.999;
        // An explicit edge handle is more reliable than relying on the
        // side-panel splitter, which can be overpainted by the content child.
        if !animating && self.show_sidebar && !self.overlay_open(ctx) {
            let edge = 46.0 + self.settings.sidebar_width;
            let pointer = ctx.input(|i| i.pointer.interact_pos());
            let over_edge = pointer
                .map(|p| (p.x - edge).abs() <= 4.0 && p.y > 34.0)
                .unwrap_or(false);
            if self.sidebar_resizing {
                if ctx.input(|i| i.pointer.primary_down()) {
                    if let Some(p) = pointer {
                        let width = (p.x - 46.0).clamp(190.0, 520.0);
                        if (width - self.settings.sidebar_width).abs() > 0.5 {
                            self.settings.sidebar_width = width;
                            self.settings_dirty = true;
                        }
                    }
                } else {
                    self.sidebar_resizing = false;
                }
            } else if over_edge && ctx.input(|i| i.pointer.primary_pressed()) {
                self.sidebar_resizing = true;
            }
            if over_edge || self.sidebar_resizing {
                ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
        }
        let full_width = self.settings.sidebar_width.clamp(190.0, 520.0);
        let width = (full_width * t).max(1.0);
        let id = if animating {
            Id::new("kodemo-sidebar-anim")
        } else {
            Id::new("kodemo-sidebar")
        };
        let panel = SidePanel::left(id)
            .exact_width(width)
            .resizable(false)
            .frame(Frame::NONE.fill(palette.panel))
            .show_separator_line(true);
        let _panel = panel.show(ctx, |ui| {
            let clip = ui.clip_rect().intersect(ui.max_rect());
            // While sliding, keep the old target width so the contents do not
            // reflow every frame. Once settled, follow the width egui actually
            // gives the panel so dragging the splitter reflows it immediately.
            let content_width = if animating {
                full_width
            } else {
                ui.available_width().max(1.0)
            };
            let content_rect = egui::Rect::from_min_size(
                ui.min_rect().min,
                Vec2::new(content_width, ui.available_height().max(1.0)),
            );
            let mut content = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(content_rect)
                    .layout(Layout::top_down(Align::Min)),
            );
            content.set_clip_rect(clip);
            content.set_min_width(content_width);
            self.show_sidebar_content(&mut content);
        });
    }

    fn show_sidebar_content(&mut self, ui: &mut Ui) {
        let palette = self.palette.clone();
        let lang = self.lang();
        let title = self.activity.title(lang);
        let pad = sidebar_pad(ui);
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.label(
                RichText::new(title)
                    .size(10.5)
                    .strong()
                    .color(palette.fg_muted),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space((pad * 0.5).max(4.0));
                match self.activity {
                    Activity::Explorer => {
                        if small_icon_button(ui, Icon::Refresh, &palette, t(lang, "Refresh", "Atualizar")).clicked() {
                            self.pending_explorer = Some(ExplorerIntent::Action(ExplorerAction::Refresh));
                        }
                        if small_icon_button(ui, Icon::Collapse, &palette, t(lang, "Collapse folders", "Recolher pastas")).clicked() {
                            self.explorer.expanded.clear();
                            self.explorer.refresh();
                        }
                        if small_icon_button(ui, Icon::Folder, &palette, t(lang, "New folder", "Nova pasta")).clicked() {
                            self.pending_explorer = Some(ExplorerIntent::Action(ExplorerAction::NewFolder(self.explorer.selected_parent())));
                        }
                        if small_icon_button(ui, Icon::Plus, &palette, t(lang, "New file", "Novo arquivo")).clicked() {
                            self.pending_explorer = Some(ExplorerIntent::Action(ExplorerAction::NewFile(self.explorer.selected_parent())));
                        }
                    }
                    Activity::Search => {
                        if small_icon_button(ui, Icon::Refresh, &palette, t(lang, "Search again", "Pesquisar novamente")).clicked() {
                            self.search.last_started.clear();
                            self.search.rx = None;
                            self.start_search();
                        }
                    }
                    Activity::Git => {
                        if small_icon_button(ui, Icon::Refresh, &palette, t(lang, "Refresh", "Atualizar")).clicked() {
                            self.git.loaded_for = None;
                            self.refresh_git();
                        }
                    }
                    Activity::Ai => {
                        if small_icon_button(ui, Icon::Settings, &palette, t(lang, "AI settings", "Configurações de IA")).clicked() {
                            self.pending_command = Some(Command::OpenSettings);
                        }
                    }
                    Activity::Extensions => {}
                }
            });
        });
        ui.add_space(4.0);
        ui.painter().hline(
            ui.max_rect().x_range(),
            ui.cursor().top(),
            Stroke::new(1.0_f32, palette.border),
        );
        ui.add_space(2.0);
        match self.activity {
            Activity::Explorer => self.show_explorer_panel(ui),
            Activity::Search => self.show_search_panel(ui),
            Activity::Git => self.show_git_panel(ui),
            Activity::Extensions => self.show_extensions_panel(ui),
            Activity::Ai => self.show_ai_panel(ui),
        }
    }

    fn show_explorer_panel(&mut self, ui: &mut Ui) {
        let palette = self.palette.clone();
        let lang = self.lang();
        let show_hidden = self.settings.show_hidden;
        let active = self.active_path();
        let pad = sidebar_pad(ui);
        let mut action = None;
        let scroll_height = (ui.available_height() - 30.0).max(60.0);
        ScrollArea::vertical()
            .id_salt("kodemo-explorer")
            .auto_shrink([false, false])
            .max_height(scroll_height)
            .show(ui, |ui| {
                ui.add_space(2.0);
                action = self.explorer.show(
                    ui,
                    &palette,
                    lang,
                    show_hidden,
                    active.as_deref(),
                    pad,
                );
                ui.add_space(10.0);
            });
        if let Some(action) = action {
            self.pending_explorer = Some(ExplorerIntent::Action(action));
        }
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_space(pad);
                let hidden = ui.selectable_label(
                    show_hidden,
                    RichText::new(t(lang, "Hidden files", "Arquivos ocultos")).size(10.5),
                );
                if hidden.clicked() {
                    self.pending_explorer =
                        Some(ExplorerIntent::Action(ExplorerAction::ToggleHidden));
                }
            });
            ui.add_space(3.0);
            ui.painter().hline(
                ui.max_rect().x_range(),
                ui.cursor().top(),
                Stroke::new(1.0_f32, palette.border),
            );
            ui.add_space(1.0);
        });
    }

    fn show_search_panel(&mut self, ui: &mut Ui) {
        let palette = self.palette.clone();
        let lang = self.lang();
        let pad = sidebar_pad(ui);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            let response = ui.add(
                TextEdit::singleline(&mut self.search.query)
                    .hint_text(t(lang, "Search in files", "Pesquisar nos arquivos"))
                    .font(FontId::proportional(12.5))
                    .desired_width(f32::INFINITY)
                    .margin(Margin::symmetric(7, 5)),
            );
            if self.search.focus {
                response.request_focus();
                self.search.focus = false;
            }
            if response.changed() {
                self.search.last_started.clear();
                self.search.rx = None;
                self.search.searching = false;
                self.start_search();
            }
        });
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            if ui
                .selectable_label(self.search.case_sensitive, RichText::new("Aa").size(10.5))
                .on_hover_text(t(lang, "Match case", "Diferenciar maiúsculas"))
                .clicked()
            {
                self.search.case_sensitive = !self.search.case_sensitive;
                self.search.last_started.clear();
                self.start_search();
            }
            if ui
                .selectable_label(self.search.whole_word, RichText::new("ab").size(10.5))
                .on_hover_text(t(lang, "Whole word", "Palavra inteira"))
                .clicked()
            {
                self.search.whole_word = !self.search.whole_word;
                self.search.last_started.clear();
                self.start_search();
            }
            ui.add_space(4.0);
            if self.search.searching {
                ui.spinner();
                ui.label(
                    RichText::new(t(lang, "Searching…", "Pesquisando…"))
                        .size(10.5)
                        .color(palette.fg_faint),
                );
            } else if !self.search.hits.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        self.search.hits.len(),
                        t(lang, "results", "resultados")
                    ))
                    .size(10.5)
                    .color(palette.fg_faint),
                );
            }
        });
        ui.add_space(4.0);
        ui.separator();
        let root = self.workspace();
        let hits = self.search.hits.clone();
        let lang = self.lang();
        let mut open = None;
        ScrollArea::vertical()
            .id_salt("kodemo-search-results")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.search.query.trim().is_empty() {
                    ui.add_space(18.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(t(
                                lang,
                                "Type to search every file in the workspace.",
                                "Digite para pesquisar em todos os arquivos do workspace.",
                            ))
                            .size(11.5)
                            .color(palette.fg_faint),
                        );
                    });
                    return;
                }
                if hits.is_empty() && !self.search.searching {
                    ui.add_space(14.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(t(lang, "No results", "Nenhum resultado"))
                                .size(11.5)
                                .color(palette.fg_faint),
                        );
                    });
                    return;
                }
                let mut last_path: Option<&Path> = None;
                for hit in &hits {
                    let is_new = last_path != Some(hit.path.as_path());
                    last_path = Some(hit.path.as_path());
                    if is_new {
                        let relative = root
                            .as_deref()
                            .and_then(|r| hit.path.strip_prefix(r).ok())
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| hit.path.display().to_string());
                        ui.add_space(5.0);
                        ui.horizontal(|ui| {
                            ui.add_space(pad);
                            let name = hit
                                .path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("file");
                            ui.label(
                                RichText::new(name)
                                    .size(11.5)
                                    .strong()
                                    .color(palette.fg),
                            );
                            ui.label(
                                RichText::new(util::shorten_path(Path::new(&relative), 38))
                                    .size(9.8)
                                    .color(palette.fg_faint),
                            );
                        });
                    }
                    let label = format!("{}:{}  {}", hit.line, hit.column, hit.text);
                    let selected = false;
                    let response = result_row(
                        ui,
                        selected,
                        &label,
                        "",
                        &palette,
                        pad,
                    );
                    if response.clicked() {
                        open = Some((hit.path.clone(), hit.line));
                    }
                }
                ui.add_space(10.0);
            });
        if let Some((path, line)) = open {
            self.pending_open = Some((path, Some(line)));
        }
    }

    fn show_git_panel(&mut self, ui: &mut Ui) {
        if self.git.loaded_for.is_none() {
            self.refresh_git();
        }
        let palette = self.palette.clone();
        let lang = self.lang();
        let root = self.workspace();
        let pad = sidebar_pad(ui);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            let branch = if self.git.branch.is_empty() {
                "—".to_owned()
            } else {
                self.git.branch.clone()
            };
            inline_icon(ui, Icon::Branch, 16.0, palette.fg_muted);
            ui.label(
                RichText::new(branch)
                    .size(12.0)
                    .strong()
                    .color(palette.fg),
            );
        });
        ui.add_space(4.0);
        if let Some(err) = &self.git.error {
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.add_space(pad);
                ui.label(RichText::new(err).size(11.0).color(palette.fg_faint));
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(pad);
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(t(lang, "Initialize repository", "Iniciar repositório"))
                                .color(palette.accent_fg)
                                .size(11.0),
                        )
                        .fill(palette.accent)
                        .corner_radius(4.0),
                    )
                    .clicked()
                {
                    if let Some(root) = &root {
                        let _ = std::process::Command::new("git")
                            .arg("-C")
                            .arg(root)
                            .arg("init")
                            .output();
                        self.git.loaded_for = None;
                        self.refresh_git();
                    }
                }
            });
            return;
        }
        if self.git.changes.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                inline_icon(ui, Icon::Check, 24.0, palette.success);
                ui.label(
                    RichText::new(t(
                        lang,
                        "No changes in the working tree.",
                        "Nenhuma alteração no diretório de trabalho.",
                    ))
                    .size(11.0)
                    .color(palette.fg_faint),
                );
            });
            return;
        }
        ui.add_space(4.0);
        ui.separator();
        let changes = self.git.changes.clone();
        let mut open = None;
        ScrollArea::vertical()
            .id_salt("kodemo-git-changes")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (status, path) in &changes {
                    let code = status.trim();
                    let color = match code {
                        "D" => palette.error,
                        "A" | "??" => palette.success,
                        "M" | "R" => palette.warning,
                        _ => palette.fg_muted,
                    };
                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 23.0),
                        Sense::click(),
                    );
                    if response.hovered() {
                        ui.painter().rect_filled(rect, 0.0, palette.hover);
                    }
                    ui.painter().text(
                        Pos2::new(rect.left() + pad, rect.center().y),
                        Align2::LEFT_CENTER,
                        code,
                        FontId::monospace(10.5),
                        color,
                    );
                    ui.painter().text(
                        Pos2::new(rect.left() + pad + 24.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        util::trimmed_center(path, 46),
                        FontId::proportional(11.5),
                        palette.fg_muted,
                    );
                    if response.clicked() {
                        if let Some(root) = &root {
                            open = Some(root.join(path));
                        }
                    }
                }
            });
        if let Some(path) = open {
            self.pending_open = Some((path, None));
        }
    }

    fn show_extensions_panel(&mut self, ui: &mut Ui) {
        let palette = self.palette.clone();
        let lang = self.lang();
        let pad = sidebar_pad(ui);
        let extensions = [
            (
                Icon::Code,
                t(lang, "Rust", "Rust"),
                t(lang, "Syntax highlighting, bracket matching and snippets.", "Realce de sintaxe, pareamento de colchetes e snippets."),
            ),
            (
                Icon::Globe,
                t(lang, "Web essentials", "Essenciais da web"),
                t(lang, "HTML, CSS, JavaScript and the live preview bridge.", "HTML, CSS, JavaScript e a ponte de prévia ao vivo."),
            ),
            (
                Icon::File,
                t(lang, "Python", "Python"),
                t(lang, "Indentation-aware highlighting and docstrings.", "Realce com consciência de indentação e docstrings."),
            ),
            (
                Icon::Sparkle,
                t(lang, "Kodemo AI", "Kodemo IA"),
                t(lang, "Offline Qwen completion through llama.cpp.", "Conclusão Qwen offline via llama.cpp."),
            ),
        ];
        ScrollArea::vertical()
            .id_salt("kodemo-extensions")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(9.0);
                ui.horizontal(|ui| {
                    ui.add_space(pad);
                    ui.label(
                        RichText::new(t(lang, "Built in", "Integradas"))
                            .size(10.0)
                            .color(palette.fg_faint),
                    );
                });
                ui.add_space(5.0);
                for (icon, name, description) in extensions {
                    ui.add_space(4.0);
                    Frame::NONE
                        .fill(palette.panel_alt)
                        .corner_radius(6.0)
                        .outer_margin(Margin::symmetric(pad as i8, 0))
                        .inner_margin(Margin::symmetric(10, 9))
                        .show(ui, |ui| {
                            ui.horizontal_top(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    Vec2::splat(26.0),
                                    Sense::hover(),
                                );
                                ui.painter().rect_filled(
                                    rect,
                                    5.0,
                                    theme::alpha(palette.accent, 30),
                                );
                                icons::draw(
                                    ui.painter(),
                                    egui::Rect::from_center_size(
                                        rect.center(),
                                        Vec2::splat(15.0),
                                    ),
                                    icon,
                                    palette.accent,
                                    1.2,
                                );
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(name)
                                            .size(11.5)
                                            .strong()
                                            .color(palette.fg),
                                    );
                                    ui.label(
                                        RichText::new(description)
                                            .size(10.3)
                                            .color(palette.fg_faint),
                                    );
                                    ui.add_space(2.0);
                                    ui.label(
                                        RichText::new(t(lang, "Built-in · enabled", "Integrada · ativa"))
                                            .size(9.5)
                                            .color(palette.success),
                                    );
                                });
                            });
                        });
                }
                ui.add_space(10.0);
            });
    }

    fn show_ai_panel(&mut self, ui: &mut Ui) {
        let palette = self.palette.clone();
        let lang = self.lang();
        let pad = sidebar_pad(ui);
        let status = self.ai.status.clone();
        let detected_language = self
            .ai
            .detected_language()
            .or_else(|| self.active_doc().map(|doc| doc.language))
            .map(|language| language.display())
            .unwrap_or("—");
        let suggestion = self.ai.suggestion.clone();
        let (status_color, status_text) = match &status {
            AiStatus::Ready => (palette.success, t(lang, "Ready", "Pronta").to_owned()),
            AiStatus::Thinking => (palette.warning, t(lang, "Thinking…", "Pensando…").to_owned()),
            AiStatus::StartingServer => (
                palette.warning,
                t(lang, "Starting llama.cpp…", "Iniciando llama.cpp…").to_owned(),
            ),
            AiStatus::Off => (palette.fg_faint, t(lang, "Off", "Desligada").to_owned()),
            AiStatus::Unavailable(_) => (
                palette.error,
                t(lang, "Unavailable", "Indisponível").to_owned(),
            ),
        };
        let elapsed = self.ai.elapsed();
        let elapsed_text = elapsed
            .map(|value| format!(" · {:.1}s", value.as_secs_f32()))
            .unwrap_or_default();
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.label(
                RichText::new(format!("● {status_text}{elapsed_text}"))
                    .size(11.5)
                    .color(status_color),
            );
            if status.is_busy() {
                ui.spinner();
            }
        });
        if status.is_busy() {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(pad);
                let width = (ui.available_width() - pad).max(40.0);
                ui.add(
                    egui::ProgressBar::new(0.35)
                        .desired_width(width)
                        .animate(true)
                        .fill(palette.accent),
                );
            });
        }
        ui.add_space(6.0);
        Frame::NONE
            .fill(palette.panel_alt)
            .corner_radius(6.0)
            .outer_margin(Margin::symmetric(pad as i8, 0))
            .inner_margin(Margin::symmetric(10, 9))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(t(
                        lang,
                        "Qwen 0.5B completes the line you are writing. Everything stays on this machine.",
                        "O Qwen 0.5B completa a linha que você está escrevendo. Tudo fica nesta máquina.",
                    ))
                    .size(10.8)
                    .color(palette.fg_muted),
                );
                ui.add_space(5.0);
                ui.label(
                    RichText::new(format!(
                        "{}: {}",
                        t(lang, "Detected language", "Linguagem detectada"),
                        detected_language
                    ))
                    .size(10.0)
                    .color(palette.accent),
                );
                ui.label(
                    RichText::new(format!(
                        "{}: {}",
                        t(lang, "Model", "Modelo"),
                        self.settings.ai_model
                    ))
                    .size(10.0)
                    .color(palette.fg_faint),
                );
                ui.label(
                    RichText::new(format!(
                        "{}: {}",
                        t(lang, "Endpoint", "Endpoint"),
                        self.ai.endpoint()
                    ))
                    .size(10.0)
                    .color(palette.fg_faint),
                );
                if let Some(latency) = self.ai.last_latency() {
                    ui.label(
                        RichText::new(format!(
                            "{}: {:.1}s",
                            t(lang, "Last completion", "Última conclusão"),
                            latency.as_secs_f32()
                        ))
                        .size(10.0)
                        .color(palette.fg_faint),
                    );
                }
                if let Some(suggestion) = &suggestion {
                    ui.add_space(5.0);
                    ui.label(
                        RichText::new(t(lang, "Latest suggestion", "Última sugestão"))
                            .size(9.6)
                            .color(palette.fg_faint),
                    );
                    for line in suggestion.text.lines().take(3) {
                        ui.label(
                            RichText::new(line)
                                .font(FontId::monospace(10.2))
                                .color(palette.fg_muted),
                        );
                    }
                }
                if let AiStatus::Unavailable(err) = &status {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(util::trimmed_center(err, 180))
                            .size(9.8)
                            .color(palette.error),
                    );
                }
            });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            let mut enabled = self.settings.ai_enabled;
            if ui
                .checkbox(&mut enabled, t(lang, "Enable AI", "Ativar IA"))
                .changed()
            {
                self.settings.ai_enabled = enabled;
                self.ai.configure(&self.settings);
                self.settings_dirty = true;
            }
            let mut ghost = self.settings.ghost;
            if ui
                .checkbox(&mut ghost, t(lang, "Ghost text", "Texto fantasma"))
                .changed()
            {
                self.settings.ghost = ghost;
                self.invalidate_ai();
                self.settings_dirty = true;
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(t(lang, "Restart server", "Reiniciar servidor")).size(10.5),
                    )
                    .fill(palette.panel_alt)
                    .corner_radius(4.0),
                )
                .clicked()
            {
                self.pending_command = Some(Command::RestartAi);
            }
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(t(lang, "AI settings", "Configurações de IA")).size(10.5),
                    )
                    .fill(palette.panel_alt)
                    .corner_radius(4.0),
                )
                .clicked()
            {
                self.pending_command = Some(Command::OpenSettings);
            }
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.label(
                RichText::new(t(lang, "Press Tab to accept a suggestion.", "Pressione Tab para aceitar uma sugestão."))
                    .size(10.2)
                    .color(palette.fg_faint),
            );
        });
    }

    fn show_bottom_panel(&mut self, ctx: &egui::Context) {
        let palette = self.palette.clone();
        let lang = self.lang();
        let mut close = false;
        let panel = TopBottomPanel::bottom("kodemo-bottom-panel")
            .resizable(true)
            .default_height(self.settings.terminal_height.max(150.0))
            .min_height(110.0)
            .max_height(460.0)
            .frame(Frame::NONE.fill(palette.panel))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.horizontal(|ui| {
                    ui.add_space(7.0);
                    for (tab, label) in [
                        (PanelTab::Problems, t(lang, "PROBLEMS", "PROBLEMAS")),
                        (PanelTab::Output, t(lang, "OUTPUT", "SAÍDA")),
                        (PanelTab::Terminal, t(lang, "TERMINAL", "TERMINAL")),
                    ] {
                        let active = self.panel_tab == tab;
                        let text = RichText::new(label)
                            .size(10.2)
                            .strong()
                            .color(if active { palette.fg } else { palette.fg_faint });
                        let mut button = egui::Button::new(text)
                            .frame(false)
                            .corner_radius(3.0)
                            .min_size(Vec2::new(0.0, 24.0));
                        if active {
                            button = button.fill(palette.panel_alt);
                        }
                        if ui.add(button).clicked() {
                            self.panel_tab = tab;
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(5.0);
                        if small_icon_button(ui, Icon::Close, &palette, t(lang, "Close panel", "Fechar painel")).clicked() {
                            close = true;
                        }
                        if small_icon_button(
                            ui,
                            Icon::Refresh,
                            &palette,
                            t(lang, "Clear", "Limpar"),
                        )
                        .clicked()
                        {
                            match self.panel_tab {
                                PanelTab::Terminal => self.terminal.clear(),
                                PanelTab::Output => self.output.clear(),
                                PanelTab::Problems => {}
                            }
                        }
                    });
                });
                ui.painter().hline(
                    ui.max_rect().x_range(),
                    ui.cursor().top(),
                    Stroke::new(1.0_f32, palette.border),
                );
                ui.add_space(3.0);
                match self.panel_tab {
                    PanelTab::Terminal => self.terminal.show(ui, &palette),
                    PanelTab::Output => {
                        ScrollArea::vertical()
                            .id_salt("kodemo-output")
                            .stick_to_bottom(true)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if self.output.is_empty() {
                                    ui.label(
                                        RichText::new(t(
                                            lang,
                                            "Kodemo output will appear here.",
                                            "A saída do Kodemo aparecerá aqui.",
                                        ))
                                        .size(11.0)
                                        .color(palette.fg_faint),
                                    );
                                }
                                for line in &self.output {
                                    ui.label(
                                        RichText::new(line)
                                            .font(FontId::monospace(11.0))
                                            .color(palette.fg_muted),
                                    );
                                }
                            });
                    }
                    PanelTab::Problems => {
                        let mut open = None;
                        ScrollArea::vertical()
                            .id_salt("kodemo-problems")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let mut any = false;
                                for (index, doc) in self.docs.iter().enumerate() {
                                    if doc.modified() {
                                        any = true;
                                        let text = format!(
                                            "● {} — {}",
                                            doc.title,
                                            t(lang, "unsaved changes", "alterações não salvas")
                                        );
                                        let response = result_row(
                                            ui,
                                            false,
                                            &text,
                                            "",
                                            &palette,
                                            10.0,
                                        );
                                        if response.clicked() {
                                            open = Some(index);
                                        }
                                    }
                                }
                                if let AiStatus::Unavailable(err) = &self.ai.status {
                                    any = true;
                                    ui.label(
                                        RichText::new(format!("● {}", util::trimmed_center(err, 150)))
                                            .size(10.8)
                                            .color(palette.error),
                                    );
                                }
                                if !any {
                                    ui.label(
                                        RichText::new(t(
                                            lang,
                                            "No problems detected.",
                                            "Nenhum problema detectado.",
                                        ))
                                        .size(11.0)
                                        .color(palette.success),
                                    );
                                }
                            });
                        if let Some(index) = open {
                            self.active = index;
                            self.invalidate_ai();
                        }
                    }
                }
                // Fill any leftover space in the panel. egui sizes a panel
                // from its content, and the terminal body computes its height
                // from the space available: without this the two rounded
                // differently and the panel crept smaller on every repaint
                // (which a mouse hover over the editor triggered).
                let leftover = ui.available_height();
                if leftover > 0.0 {
                    ui.add_space(leftover);
                }
            });
        if close {
            self.show_panel = false;
        }
        // Persist the height once the drag ended. Writing the measured height
        // back every frame fed the layout into itself, which made the terminal
        // creep smaller on each repaint.
        let height = panel.response.rect.height();
        let near_edge = ctx.input(|i| {
            i.pointer.any_released()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| (p.y - panel.response.rect.top()).abs() <= 10.0)
        });
        if near_edge && (height - self.settings.terminal_height).abs() > 1.0 {
            self.settings.terminal_height = height;
            self.settings_dirty = true;
        }
    }

    fn show_preview_panel(&mut self, ctx: &egui::Context) {
        self.ensure_preview();
        self.sync_preview_target(false);
        let palette = self.palette.clone();
        let lang = self.lang();
        let mut go = false;
        let mut close = false;
        let mut retry = false;
        SidePanel::right("kodemo-preview-panel")
            .resizable(true)
            .default_width(520.0)
            .width_range(280.0..=980.0)
            .frame(Frame::NONE.fill(palette.panel))
            .show(ctx, |ui| {
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.add_space(9.0);
                    let status_color = match &self.preview.status {
                        PreviewStatus::Ready => palette.success,
                        PreviewStatus::Starting => palette.warning,
                        PreviewStatus::Failed(_) => palette.error,
                        PreviewStatus::Idle => palette.fg_faint,
                    };
                    ui.label(RichText::new("●").size(10.0).color(status_color));
                    ui.label(
                        RichText::new(t(lang, "LIVE PREVIEW", "PRÉVIA AO VIVO"))
                            .size(10.2)
                            .strong()
                            .color(palette.fg_muted),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(4.0);
                        if small_icon_button(ui, Icon::Close, &palette, t(lang, "Close preview", "Fechar prévia")).clicked() {
                            close = true;
                        }
                        if small_icon_button(
                            ui,
                            Icon::External,
                            &palette,
                            t(lang, "Open in browser", "Abrir no navegador"),
                        )
                        .clicked()
                        {
                            self.pending_command = Some(Command::OpenInBrowser);
                        }
                        if small_icon_button(
                            ui,
                            Icon::Refresh,
                            &palette,
                            t(lang, "Reload", "Recarregar"),
                        )
                        .clicked()
                        {
                            self.preview.reload();
                        }
                    });
                });
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    let edit_width = (ui.available_width() - 52.0).max(90.0);
                    let response = ui.add(
                        TextEdit::singleline(&mut self.preview_url_input)
                            .font(FontId::monospace(11.0))
                            .desired_width(edit_width)
                            .margin(Margin::symmetric(6, 4))
                            .hint_text("http://127.0.0.1:8080/"),
                    );
                    if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        go = true;
                    }
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(t(lang, "Go", "Ir"))
                                    .size(10.5)
                                    .color(palette.accent_fg),
                            )
                            .fill(palette.accent)
                            .corner_radius(4.0),
                        )
                        .clicked()
                    {
                        go = true;
                    }
                });
                ui.add_space(4.0);
                let rect = ui.available_rect_before_wrap();
                if rect.height() > 20.0 {
                    ui.painter().rect_filled(rect, 0.0, palette.editor);
                    let failed = match &self.preview.status {
                        PreviewStatus::Failed(err) => Some(err.clone()),
                        _ => None,
                    };
                    if let Some(err) = failed {
                        let mut child = ui.new_child(
                            egui::UiBuilder::new()
                                .max_rect(rect.shrink(18.0))
                                .layout(Layout::top_down(Align::Center)),
                        );
                        child.add_space((rect.height() * 0.28).max(10.0));
                        icons::draw(
                            child.painter(),
                            egui::Rect::from_center_size(
                                child.cursor().min + Vec2::new(11.0, 10.0),
                                Vec2::splat(18.0),
                            ),
                            Icon::Warning,
                            palette.warning,
                            1.3,
                        );
                        child.add_space(22.0);
                        child.label(
                            RichText::new(t(
                                lang,
                                "The native preview could not start.",
                                "Não foi possível iniciar a prévia nativa.",
                            ))
                            .size(12.0)
                            .strong()
                            .color(palette.fg),
                        );
                        child.add_space(6.0);
                        child.label(
                            RichText::new(util::trimmed_center(&err, 260))
                                .size(10.5)
                                .color(palette.fg_faint),
                        );
                        child.add_space(12.0);
                        if child
                            .add(
                                egui::Button::new(
                                    RichText::new(t(lang, "Try again", "Tentar novamente"))
                                        .color(palette.accent_fg),
                                )
                                .fill(palette.accent)
                                .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            retry = true;
                        }
                        child.add_space(8.0);
                        if child
                            .add(
                                egui::Button::new(
                                    RichText::new(t(
                                        lang,
                                        "Open in the system browser",
                                        "Abrir no navegador do sistema",
                                    ))
                                    .size(11.0),
                                )
                                .fill(palette.panel_alt)
                                .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            self.pending_command = Some(Command::OpenInBrowser);
                        }
                    } else if self.preview.status == PreviewStatus::Idle
                        || self.preview.status == PreviewStatus::Starting
                    {
                        ui.allocate_ui_with_layout(
                            rect.size(),
                            Layout::top_down(Align::Center),
                            |ui| {
                                ui.add_space((rect.height() * 0.35).max(10.0));
                                ui.spinner();
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(t(
                                        lang,
                                        "Starting the embedded WebKit preview…",
                                        "Iniciando a prévia WebKit incorporada…",
                                    ))
                                    .size(11.0)
                                    .color(palette.fg_faint),
                                );
                            },
                        );
                    }
                    if !matches!(self.preview.status, PreviewStatus::Failed(_)) {
                        self.pending_preview_rect = Some(rect);
                    }
                }
                if self.preview_server.is_none() {
                    if let Some(err) = &self.preview_error {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(util::trimmed_center(err, 220))
                                .size(10.0)
                                .color(palette.error),
                        );
                    }
                }
            });
        if close {
            self.show_preview = false;
            self.preview.stop();
        }
        if go {
            let url = self.preview_url_input.trim().to_owned();
            if !url.is_empty() {
                self.preview_url = url.clone();
                self.preview_url_input = url;
                self.preview.reload();
                self.log(format!("Preview → {}", self.preview_url));
            }
        }
        if retry {
            self.preview_error = None;
            self.preview.reload();
            self.ensure_preview();
        }
    }
}

impl KodemoApp {
    fn show_editor_area(&mut self, ctx: &egui::Context) {
        let palette = self.palette.clone();
        CentralPanel::default()
            .frame(Frame::NONE.fill(palette.editor))
            .show(ctx, |ui| {
                self.show_tabs(ui);
                self.show_breadcrumbs(ui);
                self.show_editor_body(ui);
            });
    }

    fn show_tabs(&mut self, ui: &mut Ui) {
        let palette = self.palette.clone();
        let lang = self.lang();
        let tabs: Vec<(usize, u64, String, bool, Color32)> = self
            .docs
            .iter()
            .enumerate()
            .map(|(index, doc)| {
                (
                    index,
                    doc.id,
                    doc.title.clone(),
                    doc.modified(),
                    language_color(doc.language, &palette),
                )
            })
            .collect();
        let mut activate = None;
        let mut close = None;
        let mut new_file = false;
        let mut toggle_preview = false;
        let mut close_all = false;
        let mut close_project = false;
        let full_width = ui.available_width();
        Frame::NONE
            .fill(palette.titlebar)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let strip_width = (full_width - 124.0).max(160.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(strip_width, 34.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ScrollArea::horizontal()
                                .id_salt("kodemo-editor-tabs")
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 1.0;
                                        for (index, doc_id, title, modified, color) in &tabs {
                                            let active = *index == self.active;
                                            let fill = if active {
                                                palette.tab_active
                                            } else {
                                                palette.tab_inactive
                                            };
                                            let frame = Frame::NONE
                                                .fill(fill)
                                                .corner_radius(CornerRadius {
                                                    nw: 6,
                                                    ne: 6,
                                                    sw: 0,
                                                    se: 0,
                                                })
                                                .inner_margin(Margin::symmetric(10, 8))
                                                .show(ui, |ui| {
                                                    ui.push_id(doc_id, |ui| {
                                                        ui.horizontal(|ui| {
                                                            ui.spacing_mut().item_spacing.x = 6.0;
                                                            let (dot, _) = ui.allocate_exact_size(
                                                                Vec2::splat(7.0),
                                                                Sense::hover(),
                                                            );
                                                            ui.painter().circle_filled(
                                                                dot.center(),
                                                                2.6,
                                                                *color,
                                                            );
                                                            let text = if *modified {
                                                                format!("● {title}")
                                                            } else {
                                                                title.clone()
                                                            };
                                                            let label = ui.add(
                                                                egui::Button::new(
                                                                    RichText::new(text)
                                                                        .size(12.0)
                                                                        .color(if active {
                                                                            palette.tab_fg
                                                                        } else {
                                                                            palette.tab_fg_inactive
                                                                        }),
                                                                )
                                                                .frame(false)
                                                                .corner_radius(0.0),
                                                            );
                                                            if label.clicked() {
                                                                activate = Some(*index);
                                                            }
                                                            if icon_button(
                                                                ui,
                                                                Icon::Close,
                                                                15.0,
                                                                &palette,
                                                                t(lang, "Close file", "Fechar arquivo"),
                                                            )
                                                            .clicked()
                                                            {
                                                                close = Some(*index);
                                                            }
                                                        });
                                                    });
                                                });
                                            if active {
                                                let rect = frame.response.rect;
                                                ui.painter().hline(
                                                    rect.x_range(),
                                                    rect.bottom() - 1.0,
                                                    Stroke::new(2.0_f32, palette.accent),
                                                );
                                            }
                                        }
                                    });
                                });
                        },
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(4.0);
                        if small_icon_button(
                            ui,
                            Icon::Preview,
                            &palette,
                            t(lang, "Toggle live preview", "Alternar prévia ao vivo"),
                        )
                        .clicked()
                        {
                            toggle_preview = true;
                        }
                        if small_icon_button(
                            ui,
                            Icon::Plus,
                            &palette,
                            t(lang, "New file", "Novo arquivo"),
                        )
                        .clicked()
                        {
                            new_file = true;
                        }
                        if !tabs.is_empty()
                            && small_icon_button(
                                ui,
                                Icon::Close,
                                &palette,
                                t(lang, "Close all files and the project", "Fechar todos os arquivos e o projeto"),
                            )
                            .clicked()
                        {
                            close_all = true;
                        }
                        if small_icon_button(
                            ui,
                            Icon::Folder,
                            &palette,
                            t(lang, "Close project", "Fechar projeto"),
                        )
                        .clicked()
                        {
                            close_project = true;
                        }
                    });
                });
                let rect = ui.max_rect();
                ui.painter().hline(
                    rect.x_range(),
                    rect.bottom() - 0.5,
                    Stroke::new(1.0_f32, palette.border),
                );
            });
        if let Some(index) = activate {
            self.active = index;
            self.invalidate_ai();
            self.sync_preview_target(true);
        }
        if let Some(index) = close {
            self.pending_close_tab = Some(index);
        }
        if close_all {
            self.pending_command = Some(Command::CloseAll);
        }
        if close_project {
            self.pending_command = Some(Command::CloseProject);
        }
        if new_file {
            self.new_untitled();
        }
        if toggle_preview {
            self.pending_command = Some(Command::TogglePreview);
        }
    }

    fn show_breadcrumbs(&mut self, ui: &mut Ui) {
        let palette = self.palette.clone();
        let lang = self.lang();
        let mut parts: Vec<String> = Vec::new();
        if let Some(path) = self.active_path() {
            let relative = self
                .workspace()
                .and_then(|root| path.strip_prefix(&root).ok().map(|p| p.to_path_buf()))
                .unwrap_or(path);
            for component in relative.components() {
                parts.push(component.as_os_str().to_string_lossy().to_string());
            }
        }
        let hint = if matches!(self.ai.status, AiStatus::Thinking | AiStatus::StartingServer) {
            let language = self
                .active_doc()
                .map(|doc| doc.language.display())
                .unwrap_or("code");
            let elapsed = self
                .ai
                .elapsed()
                .map(|elapsed| format!(" · {:.1}s", elapsed.as_secs_f32()))
                .unwrap_or_default();
            let action = if self.ai.status == AiStatus::Thinking {
                if lang.is_pt() { "gerando" } else { "generating" }
            } else if lang.is_pt() {
                "iniciando"
            } else {
                "starting"
            };
            Some(format!("Qwen · {language} · {action}{elapsed}"))
        } else if self.ai.suggestion.is_some() {
            let language = self
                .active_doc()
                .map(|doc| doc.language.display())
                .unwrap_or("code");
            Some(if lang.is_pt() {
                format!("Qwen · {language} · Tab aceita a sugestão")
            } else {
                format!("Qwen · {language} · Tab accepts")
            })
        } else if self.completion.open {
            Some(
                t(
                    lang,
                    "↑↓ choose · Tab or Enter accepts",
                    "↑↓ escolher · Tab ou Enter aceita",
                )
                .to_owned(),
            )
        } else {
            None
        };
        Frame::NONE
            .fill(palette.editor)
            .inner_margin(Margin::symmetric(10, 3))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if parts.is_empty() {
                        ui.label(
                            RichText::new(t(lang, "No file open", "Nenhum arquivo aberto"))
                                .size(10.5)
                                .color(palette.fg_faint),
                        );
                    }
                    for (index, part) in parts.iter().enumerate() {
                        if index > 0 {
                            ui.label(
                                RichText::new("›")
                                    .size(10.5)
                                    .color(palette.fg_faint),
                            );
                        }
                        ui.label(
                            RichText::new(part)
                                .size(10.5)
                                .color(if index + 1 == parts.len() {
                                    palette.fg_muted
                                } else {
                                    palette.fg_faint
                                }),
                        );
                    }
                    if let Some(hint) = hint {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(hint)
                                    .size(9.8)
                                    .color(palette.accent),
                            );
                        });
                    }
                });
                let rect = ui.max_rect();
                ui.painter().hline(
                    rect.x_range(),
                    rect.bottom() - 0.5,
                    Stroke::new(1.0_f32, palette.border),
                );
            });
    }

    fn show_editor_body(&mut self, ui: &mut Ui) {
        if self.docs.is_empty() {
            self.show_welcome_editor(ui);
            return;
        }
        let index = self.active;
        let palette = self.palette.clone();
        let lang = self.lang();
        let doc_id = self.docs[index].id;
        let language = self.docs[index].language;
        let mut text = std::mem::take(&mut self.docs[index].text);
        let mut cursor_char = self.docs[index].cursor_char;
        let pending_cursor = self.docs[index].pending_cursor.take();
        let mut edited = false;
        let font = FontId::monospace(self.settings.font_size);
        let wrap = self.settings.word_wrap;
        let show_numbers = self.settings.line_numbers;
        let show_minimap = self.settings.minimap && ui.available_width() > 840.0;
        let id = editor_id(doc_id);
        let spell_issues = self.spell_issues(doc_id, &text, language);
        let mut caret_anchor: Option<Pos2> = None;
        let mut editor_focused = false;
        // Kept for one frame so a single typed character can be paired with its
        // closing bracket, quote or HTML tag.
        let previous_text = if self.settings.auto_close {
            Some(text.clone())
        } else {
            None
        };
        let mut force_cursor: Option<usize> = None;
        // Ghost accept/cancel is resolved after the editor text is committed,
        // otherwise the accepted line would be overwritten by this frame.
        let mut ghost_action: Option<bool> = None;
        if let Some(cursor) = pending_cursor {
            let mut state = TextEditState::load(ui.ctx(), id).unwrap_or_default();
            state
                .cursor
                .set_char_range(Some(CCursorRange::one(CCursor::new(cursor))));
            state.store(ui.ctx(), id);
        }
        let line_count = text.lines().count().max(1);
        let digits = line_count.to_string().len().max(2) as f32;
        let gutter_width = if show_numbers {
            16.0 + digits * self.settings.font_size * 0.62
        } else {
            0.0
        };
        let full = ui.available_rect_before_wrap();
        let minimap_width = if show_minimap { 64.0 } else { 0.0 };
        let editor_width = (full.width() - minimap_width - 1.0).max(200.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.allocate_ui_with_layout(
                Vec2::new(editor_width, full.height()),
                Layout::top_down(Align::Min),
                |ui| {
                    ScrollArea::both()
                        .id_salt(("kodemo-editor-scroll", doc_id))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_min_width(editor_width.max(100.0));
                            ui.add_space(8.0);
                            ui.horizontal_top(|ui| {
                                let gutter_left = ui.cursor().left();
                                if show_numbers {
                                    ui.add_space(gutter_width);
                                }
                                let mut cache = self.highlight_cache.take();
                                let mut layouter = |ui: &Ui, text: &str, wrap_width: f32| {
                                    let width = if wrap { wrap_width } else { f32::INFINITY };
                                    let hash = util::fnv1a(text.as_bytes());
                                    if let Some(cached) = &cache {
                                        if cached.doc_id == doc_id
                                            && cached.text_hash == hash
                                            && cached.wrap_width == width
                                        {
                                            return cached.galley.clone();
                                        }
                                    }
                                    let job = syntax::highlight(
                                        text,
                                        language,
                                        font.clone(),
                                        width,
                                        &palette,
                                    );
                                    let galley = ui.fonts(|f| f.layout_job(job));
                                    cache = Some(HighlightCache {
                                        doc_id,
                                        text_hash: hash,
                                        wrap_width: width,
                                        galley: galley.clone(),
                                    });
                                    galley
                                };
                                let desired = (ui.available_width() - 12.0).max(280.0);
                                let output = TextEdit::multiline(&mut text)
                                    .id(id)
                                    .font(font.clone())
                                    .desired_width(desired)
                                    .desired_rows(28)
                                    .frame(false)
                                    .margin(Margin::ZERO)
                                    .lock_focus(true)
                                    .hint_text(t(lang, "Start typing…", "Comece a digitar…"))
                                    .layouter(&mut layouter)
                                    .show(ui);
                                self.highlight_cache = cache;
                                edited = output.response.changed();
                                editor_focused = output.response.has_focus();
                                let jump = pending_cursor.is_some();
                                let mut current = cursor_char;
                                if let Some(range) = output.cursor_range {
                                    current = range.as_ccursor_range().primary.index;
                                }
                                if edited {
                                    if let Some(previous) = &previous_text {
                                        if let Some(adjusted) =
                                            auto_close_edit(&mut text, previous, current, language)
                                        {
                                            current = adjusted;
                                            force_cursor = Some(adjusted);
                                            ui.ctx().request_repaint();
                                        }
                                    }
                                }
                                if let Some(suggestion) = self.ai.suggestion.clone() {
                                    if suggestion.start == current {
                                        let cursor =
                                            output.galley.from_ccursor(CCursor::new(current));
                                        let caret = output.galley.pos_from_cursor(&cursor);
                                        let start_pos =
                                            caret.min + output.galley_pos.to_vec2();
                                        let clip = output.text_clip_rect;
                                        let painter = ui.painter().with_clip_rect(clip);
                                        let ghost_color = theme::alpha(palette.fg_faint, 170);
                                        // Paint the first line at the caret and the
                                        // rest at the line start, so a multi-line
                                        // suggestion keeps the code's indentation
                                        // and line breaks instead of running on.
                                        let (first_line, rest) = match suggestion.text.find('\n') {
                                            Some(index) => (
                                                &suggestion.text[..index],
                                                Some(&suggestion.text[index + 1..]),
                                            ),
                                            None => (suggestion.text.as_str(), None),
                                        };
                                        let first = painter.layout_no_wrap(
                                            first_line.to_owned(),
                                            font.clone(),
                                            ghost_color,
                                        );
                                        painter.galley(start_pos, first.clone(), ghost_color);
                                        let mut ghost_rect = egui::Rect::from_min_size(
                                            start_pos,
                                            first.size(),
                                        );
                                        if let Some(rest) = rest {
                                            let left = output.galley_pos.x;
                                            let wrap_width =
                                                (clip.right() - left - 6.0).max(80.0);
                                            let rest_galley = painter.layout(
                                                rest.to_owned(),
                                                font.clone(),
                                                ghost_color,
                                                wrap_width,
                                            );
                                            let rest_pos = Pos2::new(
                                                left,
                                                start_pos.y + first.size().y,
                                            );
                                            painter.galley(
                                                rest_pos,
                                                rest_galley.clone(),
                                                ghost_color,
                                            );
                                            ghost_rect = ghost_rect.union(
                                                egui::Rect::from_min_size(
                                                    rest_pos,
                                                    rest_galley.size(),
                                                ),
                                            );
                                        }
                                        let ghost_rect = ghost_rect.intersect(clip);
                                        let pointer = ui.ctx().pointer_hover_pos();
                                        let over_ghost = ghost_rect.is_positive()
                                            && pointer.is_some_and(|p| {
                                                ghost_rect.expand(2.0).contains(p)
                                            });
                                        let over_controls = pointer.is_some_and(|p| {
                                            self.ghost_controls_rect
                                                .is_some_and(|rect| rect.expand(2.0).contains(p))
                                        });
                                        // Clicking the ghost text itself accepts it,
                                        // exactly like pressing Tab.
                                        if over_ghost
                                            && ui.input(|i| i.pointer.primary_clicked())
                                        {
                                            ghost_action = Some(true);
                                        }
                                        if over_ghost || over_controls {
                                            ui.ctx()
                                                .set_cursor_icon(egui::CursorIcon::PointingHand);
                                            let clip = output.text_clip_rect;
                                            let x = (ghost_rect.right() + 8.0)
                                                .min(clip.right() - 148.0)
                                                .max(clip.left());
                                            let y = (ghost_rect.bottom() + 4.0)
                                                .min(clip.bottom() - 32.0)
                                                .max(clip.top());
                                            let area = egui::Area::new(Id::new((
                                                "kodemo-ghost-controls",
                                                doc_id,
                                            )))
                                            .order(egui::Order::Foreground)
                                            .fixed_pos(Pos2::new(x, y))
                                            .constrain(true)
                                            .show(ui.ctx(), |ui| {
                                                Frame::popup(ui.style())
                                                    .fill(palette.panel)
                                                    .stroke(Stroke::new(
                                                        1.0_f32,
                                                        palette.border_strong,
                                                    ))
                                                    .corner_radius(6)
                                                    .inner_margin(Margin::symmetric(4, 3))
                                                    .show(ui, |ui| {
                                                        ui.horizontal(|ui| {
                                                            ui.spacing_mut().item_spacing.x = 4.0;
                                                            if ui
                                                                .small_button(t(
                                                                    lang, "Accept", "Aceitar",
                                                                ))
                                                                .clicked()
                                                            {
                                                                ghost_action = Some(true);
                                                            }
                                                            if ui
                                                                .small_button(t(
                                                                    lang, "Cancel", "Cancelar",
                                                                ))
                                                                .clicked()
                                                            {
                                                                ghost_action = Some(false);
                                                            }
                                                        });
                                                    });
                                            });
                                            self.ghost_controls_rect = Some(area.response.rect);
                                        } else {
                                            self.ghost_controls_rect = None;
                                        }
                                    }
                                }
                                {
                                    let cursor = output.galley.from_ccursor(CCursor::new(current));
                                    let caret = output.galley.pos_from_cursor(&cursor);
                                    caret_anchor = Some(
                                        output.galley_pos
                                            + caret.left_bottom().to_vec2()
                                            + Vec2::new(0.0, 2.0),
                                    );
                                    // Search results and "Go to line" jumps must
                                    // actually scroll the target line into view.
                                    if jump {
                                        output.response.request_focus();
                                        let rect = egui::Rect::from_min_size(
                                            output.galley_pos + caret.min.to_vec2(),
                                            Vec2::new(1.0, caret.height().max(4.0)),
                                        );
                                        ui.scroll_to_rect(
                                            rect.expand2(Vec2::new(0.0, self.settings.font_size)),
                                            Some(Align::Center),
                                        );
                                    }
                                }
                                draw_spell_underlines(
                                    ui,
                                    &output.galley,
                                    output.galley_pos,
                                    output.text_clip_rect,
                                    &spell_issues,
                                    current,
                                    palette.error,
                                );
                                cursor_char = current;
                                if show_numbers {
                                    let top = output.galley_pos.y;
                                    let bottom = (top + output.galley.size().y)
                                        .max(top + ui.available_height());
                                    let gutter = egui::Rect::from_min_max(
                                        Pos2::new(gutter_left, top - 8.0),
                                        Pos2::new(gutter_left + gutter_width, bottom),
                                    );
                                    let painter = ui.painter();
                                    painter.rect_filled(gutter, 0.0, palette.editor);
                                    painter.vline(
                                        gutter.right() - 0.5,
                                        gutter.y_range(),
                                        Stroke::new(1.0_f32, palette.border),
                                    );
                                    // Clip to the gutter itself: the text clip
                                    // starts at the code, which would hide the
                                    // numbers on the left of it.
                                    let painter =
                                        painter.with_clip_rect(gutter.intersect(ui.clip_rect()));
                                    let mut line = 1usize;
                                    let mut draw_number = true;
                                    for row in &output.galley.rows {
                                        let y = top + row.rect.top();
                                        if draw_number {
                                            painter.text(
                                                Pos2::new(gutter.right() - 7.0, y),
                                                Align2::RIGHT_TOP,
                                                line.to_string(),
                                                font.clone(),
                                                palette.fg_faint,
                                            );
                                        }
                                        if row.ends_with_newline {
                                            line += 1;
                                            draw_number = true;
                                        } else {
                                            draw_number = false;
                                        }
                                    }
                                }
                            });
                            ui.add_space(24.0);
                        });
                },
            );
            if show_minimap {
                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(minimap_width, full.height()),
                    Sense::hover(),
                );
                draw_minimap(ui, rect, &text, &palette);
            }
        });
        self.docs[index].text = text;
        self.docs[index].cursor_char = cursor_char;
        if force_cursor.is_some() {
            self.docs[index].pending_cursor = force_cursor;
        }
        self.refresh_completion(ui.ctx(), doc_id, cursor_char, caret_anchor, full, editor_focused);
        match ghost_action {
            Some(true) => self.accept_ghost(),
            Some(false) => {
                self.ai.suggestion = None;
                self.ai_due = None;
                self.ghost_controls_rect = None;
            }
            None => {}
        }
        if edited {
            self.docs[index].last_edit = Instant::now();
            self.ai.suggestion = None;
            self.ai_due = None;
            self.ghost_controls_rect = None;
            // Normal typing asks for a fresh short hint again.
            self.ai.reset_boost();
            self.schedule_ai();
            ui.ctx().request_repaint();
        }
    }

    fn show_welcome_editor(&mut self, ui: &mut Ui) {
        let palette = self.palette.clone();
        let lang = self.lang();
        let mut command = None;
        let mut open = None;
        let recent_files = self.settings.recent_files.clone();
        let recent_folders = self.settings.recent_folders.clone();
        let workspace = self.workspace();
        ui.add_space(24.0);
        // Identical, responsive padding on both sides keeps the start page
        // balanced from a narrow editor to a full-screen window.
        let full_width = ui.available_width();
        let pad = (full_width * 0.07).clamp(14.0, 64.0);
        let content_width = (full_width - pad * 2.0).max(220.0);
        ui.horizontal_top(|ui| {
            ui.add_space(pad);
            ui.vertical(|ui| {
                ui.set_width(content_width);
                ui.vertical_centered(|ui| {
            if let Some(texture) = self.logo.clone() {
                ui.add(
                    egui::Image::from_texture(&texture)
                        .fit_to_exact_size(Vec2::splat(92.0)),
                );
            }
            ui.add_space(10.0);
            ui.label(
                RichText::new("Kodemo")
                    .font(theme::display_font(30.0))
                    .color(palette.fg),
            );
            ui.label(
                RichText::new(t(
                    lang,
                    "A quiet desktop IDE, written in Rust.",
                    "Uma IDE de desktop tranquila, escrita em Rust.",
                ))
                .size(12.5)
                .color(palette.fg_muted),
            );
            ui.add_space(18.0);
            // Keep the large action cards readable when the sidebar widens
            // or the live preview narrows the editor.
            let actions = [
                (Icon::Plus, t(lang, "New file", "Novo arquivo"), Command::NewFile),
                (Icon::File, t(lang, "Open file", "Abrir arquivo"), Command::OpenFile),
                (Icon::FolderOpen, t(lang, "Open folder", "Abrir pasta"), Command::OpenFolder),
                (Icon::Command, t(lang, "Commands", "Comandos"), Command::CommandPalette),
            ];
            let gap = 6.0;
            let available = ui.available_width();
            let columns = if available >= 490.0 {
                4
            } else if available >= 242.0 {
                2
            } else {
                1
            };
            for row in actions.chunks(columns) {
                let row_width = row.len() as f32 * 118.0
                    + row.len().saturating_sub(1) as f32 * gap;
                ui.horizontal(|ui| {
                    ui.add_space(((available - row_width) * 0.5).max(0.0));
                    for (index, (icon, label, action)) in row.iter().enumerate() {
                        if index > 0 {
                            ui.add_space(gap);
                        }
                        if welcome_action(ui, *icon, label, &palette) {
                            command = Some(action.clone());
                        }
                    }
                });
                ui.add_space(gap);
            }
            ui.add_space(18.0);
            if let Some(workspace) = &workspace {
                ui.label(
                    RichText::new(t(lang, "Workspace", "Workspace"))
                        .size(10.0)
                        .color(palette.fg_faint),
                );
                ui.label(
                    RichText::new(util::shorten_path(workspace, 70))
                        .size(11.0)
                        .color(palette.fg_muted),
                );
            }
            ui.add_space(12.0);
            if !recent_files.is_empty() || !recent_folders.is_empty() {
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), 18.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.label(
                            RichText::new(t(lang, "Recent", "Recentes"))
                                .size(10.0)
                                .color(palette.fg_faint),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(t(lang, "Close All", "Fechar tudo"))
                                            .size(9.8)
                                            .color(palette.fg_faint),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text(t(
                                    lang,
                                    "Forget every recent file and folder",
                                    "Esquecer todos os arquivos e pastas recentes",
                                ))
                                .clicked()
                            {
                                command = Some(Command::ClearRecent);
                            }
                        });
                    },
                );
                ui.add_space(2.0);
                for file in recent_files.iter().take(6) {
                    let path = PathBuf::from(file);
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(file)
                        .to_owned();
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), 20.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            let row = format!(
                                "{}  {}",
                                name,
                                util::shorten_path(&path, 44)
                            );
                            if ui
                                .add(
                                    egui::Label::new(
                                        RichText::new(row)
                                            .size(11.0)
                                            .color(palette.fg_muted),
                                    )
                                    .sense(Sense::click()),
                                )
                                .clicked()
                            {
                                open = Some(path.clone());
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if small_icon_button(
                                    ui,
                                    Icon::Close,
                                    &palette,
                                    t(lang, "Remove from recent", "Remover dos recentes"),
                                )
                                .clicked()
                                {
                                    command = Some(Command::ForgetRecent(file.clone(), false));
                                }
                            });
                        },
                    );
                }
                for folder in recent_folders.iter().take(4) {
                    let path = PathBuf::from(folder);
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(folder)
                        .to_owned();
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), 20.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            let row = format!(
                                "{}  {}",
                                name,
                                util::shorten_path(&path, 44)
                            );
                            if ui
                                .add(
                                    egui::Label::new(
                                        RichText::new(row)
                                            .size(11.0)
                                            .color(palette.fg_faint),
                                    )
                                    .sense(Sense::click()),
                                )
                                .clicked()
                            {
                                command = Some(Command::OpenRecentFolder(folder.clone()));
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if small_icon_button(
                                    ui,
                                    Icon::Close,
                                    &palette,
                                    t(lang, "Remove from recent", "Remover dos recentes"),
                                )
                                .clicked()
                                {
                                    command = Some(Command::ForgetRecent(folder.clone(), true));
                                }
                            });
                        },
                    );
                }
            }
            ui.add_space(12.0);
            ui.label(
                RichText::new(t(
                    lang,
                    "Ctrl+P quick open · Ctrl+Shift+P commands · Ctrl+Alt+P live preview",
                    "Ctrl+P abrir rápido · Ctrl+Shift+P comandos · Ctrl+Alt+P prévia ao vivo",
                ))
                .size(10.0)
                .color(palette.fg_faint),
            );
                });
            });
        });
        if let Some(command) = command {
            self.pending_command = Some(command);
        }
        if let Some(path) = open {
            self.pending_open = Some((path, None));
        }
    }

    fn spell_issues(
        &mut self,
        doc_id: u64,
        text: &str,
        language: Language,
    ) -> Vec<spell::Issue> {
        if !self.settings.spellcheck {
            return Vec::new();
        }
        let text_hash = util::fnv1a(text.as_bytes());
        if let Some(cache) = &self.spell_cache {
            if cache.doc_id == doc_id && cache.text_hash == text_hash && cache.language == language {
                return cache.issues.clone();
            }
        }
        let issues = spell::issues(text, language);
        self.spell_cache = Some(SpellCache {
            doc_id,
            text_hash,
            language,
            issues: issues.clone(),
        });
        issues
    }

    fn refresh_completion(
        &mut self,
        ctx: &egui::Context,
        doc_id: u64,
        cursor: usize,
        anchor: Option<Pos2>,
        bounds: egui::Rect,
        focused: bool,
    ) {
        if !self.settings.auto_complete
            || !focused
            || self.overlay_open(ctx)
            || self.ai.suggestion.is_some()
        {
            self.completion.close();
            return;
        }
        let Some(doc) = self.docs.get(self.active) else {
            self.completion.close();
            return;
        };
        if doc.id != doc_id {
            self.completion.close();
            return;
        }
        let text_hash = util::fnv1a(doc.text.as_bytes());
        if let Some((suppress_doc, suppress_hash, suppress_cursor)) = self.completion.suppress
        {
            if suppress_doc == doc_id && suppress_hash == text_hash && suppress_cursor == cursor {
                self.completion.close();
                return;
            }
            self.completion.suppress = None;
        }
        let Some(word) = assist::word_at_cursor(&doc.text, cursor) else {
            self.completion.close();
            return;
        };
        if word.prefix.chars().count() < self.settings.completion_min_chars.max(1) {
            self.completion.close();
            return;
        }
        if self.completion.open
            && self.completion.doc_id == doc_id
            && self.completion.start == word.start
            && self.completion.prefix == word.prefix
            && self.completion.text_hash == text_hash
        {
            self.completion.anchor = anchor;
            self.completion.bounds = bounds;
            return;
        }
        let items = assist::completions(doc.language, &word.prefix, &doc.text, cursor, 8);
        if items.is_empty() {
            self.completion.close();
            return;
        }
        let same_prefix = self.completion.open
            && self.completion.doc_id == doc_id
            && self.completion.prefix == word.prefix;
        let selected = if same_prefix {
            self.completion.selected.min(items.len().saturating_sub(1))
        } else {
            0
        };
        self.completion = CompletionState {
            open: true,
            doc_id,
            start: word.start,
            prefix: word.prefix,
            text_hash,
            items,
            selected,
            anchor,
            bounds,
            suppress: self.completion.suppress,
        };
        ctx.request_repaint();
    }

    fn accept_completion(&mut self, index: usize) {
        let Some(item) = self.completion.items.get(index).cloned() else {
            self.completion.close();
            return;
        };
        let (doc_id, start, prefix) = (
            self.completion.doc_id,
            self.completion.start,
            self.completion.prefix.clone(),
        );
        let mut accepted: Option<(u64, u64, usize)> = None;
        if let Some(doc) = self.docs.get_mut(self.active) {
            if doc.id == doc_id {
                let cursor = doc.cursor_char.min(doc.text.chars().count());
                let actual: String = doc
                    .text
                    .chars()
                    .skip(start)
                    .take(cursor.saturating_sub(start))
                    .collect();
                if actual == prefix {
                    let byte_start = util::char_to_byte(&doc.text, start);
                    let byte_end = util::char_to_byte(&doc.text, cursor);
                    doc.text.replace_range(byte_start..byte_end, &item.text);
                    let cursor = start + item.text.chars().count();
                    doc.cursor_char = cursor;
                    doc.pending_cursor = Some(cursor);
                    doc.last_edit = Instant::now();
                    accepted = Some((doc_id, util::fnv1a(doc.text.as_bytes()), cursor));
                }
            }
        }
        self.completion.close();
        if let Some(state) = accepted {
            self.completion.suppress = Some(state);
            self.ai.suggestion = None;
            self.ai_due = None;
            self.invalidate_editor();
        }
    }

    fn show_completion(&mut self, ctx: &egui::Context) {
        if !self.completion.open
            || self.completion.items.is_empty()
            || self.overlay_open(ctx)
        {
            return;
        }
        let Some(anchor) = self.completion.anchor else {
            return;
        };
        let palette = self.palette.clone();
        let lang = self.lang();
        let items = self.completion.items.clone();
        let prefix = self.completion.prefix.clone();
        let selected = self.completion.selected;
        let mut clicked = None;
        let mut hovered = None;
        let screen = ctx.screen_rect();
        let width = (self.completion.bounds.width() - 12.0).clamp(238.0, 342.0);
        let row_height = 24.0;
        let height = items.len() as f32 * row_height + 26.0;
        let x = (anchor.x + 2.0)
            .clamp(screen.left() + 8.0, (screen.right() - width - 8.0).max(screen.left() + 8.0));
        let y = if anchor.y + 4.0 + height <= screen.bottom() - 8.0 {
            anchor.y + 4.0
        } else {
            (anchor.y - height - 8.0).max(screen.top() + 8.0)
        };
        egui::Area::new(Id::new("kodemo-completion-popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(x, y))
            .constrain(false)
            .show(ctx, |ui| {
                Frame::popup(ctx.style().as_ref())
                    .fill(palette.panel)
                    .stroke(Stroke::new(1.0_f32, palette.border_strong))
                    .corner_radius(8)
                    .inner_margin(Margin::symmetric(6, 5))
                    .show(ui, |ui| {
                        ui.set_width(width - 12.0);
                        ui.spacing_mut().item_spacing.y = 1.0;
                        for (index, item) in items.iter().enumerate() {
                            let (rect, response) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), row_height),
                                Sense::click(),
                            );
                            if response.hovered() {
                                hovered = Some(index);
                            }
                            if response.clicked() {
                                clicked = Some(index);
                            }
                            let fill = if index == selected {
                                theme::alpha(palette.accent, 26)
                            } else if response.hovered() {
                                palette.hover
                            } else {
                                Color32::TRANSPARENT
                            };
                            let painter = ui.painter();
                            if fill != Color32::TRANSPARENT {
                                painter.rect_filled(rect, 5.0, fill);
                            }
                            let marker = egui::Rect::from_center_size(
                                Pos2::new(rect.left() + 11.0, rect.center().y),
                                Vec2::splat(6.0),
                            );
                            painter.circle_filled(
                                marker.center(),
                                2.6,
                                if item.keyword { palette.accent } else { palette.fg_faint },
                            );
                            let font = FontId::monospace(11.8);
                            let matched_count = prefix
                                .chars()
                                .count()
                                .min(item.text.chars().count());
                            let matched: String =
                                item.text.chars().take(matched_count).collect();
                            let rest: String = item.text.chars().skip(matched_count).collect();
                            let matched_galley = painter.layout_no_wrap(
                                matched,
                                font.clone(),
                                palette.accent,
                            );
                            let rest_galley =
                                painter.layout_no_wrap(rest, font.clone(), palette.fg);
                            let text_x = rect.left() + 22.0;
                            let text_y =
                                rect.center().y - matched_galley.size().y * 0.5;
                            painter.galley(
                                Pos2::new(text_x, text_y),
                                matched_galley.clone(),
                                palette.accent,
                            );
                            painter.galley(
                                Pos2::new(text_x + matched_galley.size().x, text_y),
                                rest_galley,
                                palette.fg,
                            );
                            painter.text(
                                Pos2::new(rect.right() - 4.0, rect.center().y),
                                Align2::RIGHT_CENTER,
                                item.detail,
                                FontId::proportional(8.8),
                                palette.fg_faint,
                            );
                        }
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(t(
                                lang,
                                "Tab / Enter  accept     Esc  dismiss",
                                "Tab / Enter  aceitar     Esc  fechar",
                            ))
                            .size(8.8)
                            .color(palette.fg_faint),
                        );
                    });
            });
        if let Some(index) = clicked {
            self.accept_completion(index);
        } else if let Some(index) = hovered {
            if selected != index {
                self.completion.selected = index;
                ctx.request_repaint();
            }
        } else {
            self.completion.selected = selected;
        }
    }

    fn accept_ghost(&mut self) {
        let Some(suggestion) = self.ai.suggestion.take() else {
            return;
        };
        if suggestion.text.is_empty() {
            return;
        }
        let Some(doc) = self.docs.get_mut(self.active) else {
            return;
        };
        // Clicking the ghost text moves the caret in the same frame, so accept
        // at the position the suggestion was requested for rather than
        // requiring the caret to still be there.
        let start = suggestion.start.min(doc.text.chars().count());
        let byte = util::char_to_byte(&doc.text, start);
        doc.text.insert_str(byte, &suggestion.text);
        let cursor = start + suggestion.text.chars().count();
        doc.cursor_char = cursor;
        doc.pending_cursor = Some(cursor);
        doc.last_edit = Instant::now();
        self.ai_due = None;
        self.ghost_controls_rect = None;
        self.invalidate_editor();
        // Accepting one suggestion invites a longer one: the engine boosts its
        // budget and Kodemo asks again right away.
        self.ai.boost();
        self.schedule_ai();
        self.set_status(
            t(self.lang(), "Suggestion accepted", "Sugestão aceita"),
            StatusKind::Success,
        );
    }

    fn auto_save_docs(&mut self) {
        if !self.settings.auto_save {
            return;
        }
        let due: Vec<usize> = self
            .docs
            .iter()
            .enumerate()
            .filter(|(_, doc)| {
                doc.path.is_some()
                    && doc.modified()
                    && doc.last_edit.elapsed() > Duration::from_millis(1200)
            })
            .map(|(index, _)| index)
            .collect();
        for index in due {
            self.save_doc(index);
        }
    }
}

impl KodemoApp {
    fn show_prompt(&mut self, ctx: &egui::Context) {
        if self.prompt.is_none() {
            return;
        }
        let palette = self.palette.clone();
        let lang = self.lang();
        let mut submit = false;
        let mut cancel = false;
        let modal = Modal::new(Id::new("kodemo-prompt-dialog"))
            .frame(modal_frame(&palette))
            .backdrop_color(Color32::from_black_alpha(110))
            .show(ctx, |ui| {
                ui.set_width(430.0);
                let Some(prompt) = self.prompt.as_mut() else {
                    return;
                };
                ui.label(
                    RichText::new(&prompt.title)
                        .size(14.5)
                        .strong()
                        .color(palette.fg),
                );
                ui.add_space(8.0);
                let response = ui.add(
                    TextEdit::singleline(&mut prompt.value)
                        .font(FontId::monospace(13.0))
                        .desired_width(f32::INFINITY)
                        .margin(Margin::symmetric(7, 5)),
                );
                if prompt.just_opened {
                    response.request_focus();
                    prompt.just_opened = false;
                }
                if let Some(err) = &prompt.error {
                    ui.add_space(4.0);
                    ui.label(RichText::new(err).size(10.8).color(palette.error));
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t(lang, "Enter to confirm · Esc to cancel", "Enter para confirmar · Esc para cancelar"))
                            .size(9.8)
                            .color(palette.fg_faint),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(t(lang, "OK", "OK"))
                                        .color(palette.accent_fg),
                                )
                                .fill(palette.accent)
                                .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            submit = true;
                        }
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(t(lang, "Cancel", "Cancelar"))
                                        .color(palette.fg),
                                )
                                .fill(palette.panel_alt)
                                .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            cancel = true;
                        }
                    });
                });
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    submit = true;
                }
            });
        if modal.backdrop_response.clicked() || ctx.input(|i| i.key_pressed(Key::Escape)) {
            cancel = true;
        }
        if cancel {
            self.prompt = None;
        } else if submit {
            if let Some(prompt) = self.prompt.take() {
                self.submit_prompt(prompt);
            }
        }
    }

    fn show_confirm(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &self.confirm else {
            return;
        };
        let palette = self.palette.clone();
        let lang = self.lang();
        let kind = dialog.kind.clone();
        let message = dialog.message.clone();
        let is_delete = matches!(kind, ConfirmKind::Delete(_));
        let mut action = 0u8;
        let modal = Modal::new(Id::new("kodemo-confirm-dialog"))
            .frame(modal_frame(&palette))
            .backdrop_color(Color32::from_black_alpha(110))
            .show(ctx, |ui| {
                ui.set_width(420.0);
                ui.horizontal_top(|ui| {
                    icons::draw(
                        ui.painter(),
                        egui::Rect::from_center_size(
                            ui.cursor().min + Vec2::new(9.0, 10.0),
                            Vec2::splat(17.0),
                        ),
                        Icon::Warning,
                        if is_delete {
                            palette.error
                        } else {
                            palette.warning
                        },
                        1.3,
                    );
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(if is_delete {
                                t(lang, "Delete permanently?", "Excluir permanentemente?")
                            } else {
                                t(lang, "Unsaved changes", "Alterações não salvas")
                            })
                            .size(14.0)
                            .strong()
                            .color(palette.fg),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(&message)
                                .size(11.5)
                                .color(palette.fg_muted),
                        );
                    });
                });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        match &kind {
                            ConfirmKind::Delete(_) => {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(t(lang, "Delete", "Excluir"))
                                                .color(Color32::WHITE),
                                        )
                                        .fill(palette.error)
                                        .corner_radius(4.0),
                                    )
                                    .clicked()
                                {
                                    action = 1;
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(t(lang, "Cancel", "Cancelar"))
                                                .color(palette.fg),
                                        )
                                        .fill(palette.panel_alt)
                                        .corner_radius(4.0),
                                    )
                                    .clicked()
                                {
                                    action = 3;
                                }
                            }
                            ConfirmKind::CloseTab(_) => {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(t(lang, "Save", "Salvar"))
                                                .color(palette.accent_fg),
                                        )
                                        .fill(palette.accent)
                                        .corner_radius(4.0),
                                    )
                                    .clicked()
                                {
                                    action = 1;
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(t(lang, "Don't save", "Não salvar"))
                                                .color(palette.fg),
                                        )
                                        .fill(palette.panel_alt)
                                        .corner_radius(4.0),
                                    )
                                    .clicked()
                                {
                                    action = 2;
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(t(lang, "Cancel", "Cancelar"))
                                                .color(palette.fg_muted),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = 3;
                                }
                            }
                            ConfirmKind::CloseApp => {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(t(lang, "Save all and quit", "Salvar tudo e sair"))
                                                .color(palette.accent_fg),
                                        )
                                        .fill(palette.accent)
                                        .corner_radius(4.0),
                                    )
                                    .clicked()
                                {
                                    action = 1;
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(t(lang, "Quit without saving", "Sair sem salvar"))
                                                .color(palette.fg),
                                        )
                                        .fill(palette.panel_alt)
                                        .corner_radius(4.0),
                                    )
                                    .clicked()
                                {
                                    action = 2;
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(t(lang, "Cancel", "Cancelar"))
                                                .color(palette.fg_muted),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = 3;
                                }
                            }
                            ConfirmKind::CloseProject => {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(t(lang, "Save all and close project", "Salvar tudo e fechar projeto"))
                                                .color(palette.accent_fg),
                                        )
                                        .fill(palette.accent)
                                        .corner_radius(4.0),
                                    )
                                    .clicked()
                                {
                                    action = 1;
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(t(lang, "Close project without saving", "Fechar projeto sem salvar"))
                                                .color(palette.fg),
                                        )
                                        .fill(palette.panel_alt)
                                        .corner_radius(4.0),
                                    )
                                    .clicked()
                                {
                                    action = 2;
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(t(lang, "Cancel", "Cancelar"))
                                                .color(palette.fg_muted),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = 3;
                                }
                            }
                        }
                    });
                });
            });
        if modal.backdrop_response.clicked() || ctx.input(|i| i.key_pressed(Key::Escape)) {
            action = 3;
        }
        if action == 0 {
            return;
        }
        self.confirm = None;
        if action == 3 {
            return;
        }
        match kind {
            ConfirmKind::Delete(path) => self.confirm_action(ConfirmKind::Delete(path), ctx),
            ConfirmKind::CloseTab(index) => {
                if action == 1 {
                    let has_path = self
                        .docs
                        .get(index)
                        .map(|doc| doc.path.is_some())
                        .unwrap_or(false);
                    self.save_doc(index);
                    if has_path {
                        self.close_doc(index);
                    }
                } else {
                    self.close_doc(index);
                }
            }
            ConfirmKind::CloseProject => {
                if action == 1 {
                    self.save_all();
                }
                self.close_project();
            }
            ConfirmKind::CloseApp => {
                if action == 1 {
                    self.save_all();
                }
                self.allow_close = true;
                let _ = self.settings.save();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn show_quick_open(&mut self, ctx: &egui::Context) {
        if !self.quick.open {
            return;
        }
        let palette = self.palette.clone();
        let lang = self.lang();
        let files_mode = self.quick.files_mode;
        let workspace = self.workspace();
        let commands = command_palette(lang);
        let mut close = false;
        let mut chosen_file: Option<PathBuf> = None;
        let mut chosen_command: Option<Command> = None;
        let modal = Modal::new(Id::new("kodemo-quick-open"))
            .frame(modal_frame(&palette))
            .backdrop_color(Color32::from_black_alpha(100))
            .show(ctx, |ui| {
                ui.set_width(620.0);
                let response = ui.add(
                    TextEdit::singleline(&mut self.quick.query)
                        .font(FontId::monospace(13.0))
                        .desired_width(f32::INFINITY)
                        .hint_text(if files_mode {
                            t(lang, "Search files by name…", "Buscar arquivos pelo nome…")
                        } else {
                            t(lang, "Type a command…", "Digite um comando…")
                        })
                        .margin(Margin::symmetric(7, 6)),
                );
                if self.quick.just_opened {
                    response.request_focus();
                    self.quick.just_opened = false;
                }
                if response.changed() {
                    self.quick.selected = 0;
                }
                ui.add_space(5.0);
                ui.separator();
                let needle = self.quick.query.trim().to_lowercase();
                let mut items: Vec<(String, String, Option<Command>, Option<PathBuf>)> = Vec::new();
                if files_mode {
                    for path in self.quick.files.iter().take(5000) {
                        let relative = workspace
                            .as_deref()
                            .and_then(|root| path.strip_prefix(root).ok())
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.display().to_string());
                        if needle.is_empty()
                            || relative.to_lowercase().contains(&needle)
                        {
                            let name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("file")
                                .to_owned();
                            items.push((name, relative, None, Some(path.clone())));
                        }
                        if items.len() >= 60 {
                            break;
                        }
                    }
                } else {
                    for (label, command, shortcut) in &commands {
                        if needle.is_empty()
                            || label.to_lowercase().contains(&needle)
                        {
                            items.push((
                                label.clone(),
                                (*shortcut).to_owned(),
                                Some(command.clone()),
                                None,
                            ));
                        }
                        if items.len() >= 60 {
                            break;
                        }
                    }
                }
                if !items.is_empty() {
                    let max = items.len() - 1;
                    self.quick.selected = self.quick.selected.min(max);
                    let down = ui.input(|i| {
                        i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::PageDown)
                    });
                    let up = ui.input(|i| {
                        i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::PageUp)
                    });
                    if down {
                        self.quick.selected = (self.quick.selected + 1).min(max);
                    }
                    if up {
                        self.quick.selected = self.quick.selected.saturating_sub(1);
                    }
                }
                ScrollArea::vertical()
                    .id_salt("kodemo-quick-results")
                    .max_height(380.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if items.is_empty() {
                            ui.add_space(14.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(t(lang, "No matches", "Nenhum resultado"))
                                        .size(11.5)
                                        .color(palette.fg_faint),
                                );
                            });
                            return;
                        }
                        for (index, (title, hint, command, path)) in
                            items.iter().enumerate()
                        {
                            let response = result_row(
                                ui,
                                index == self.quick.selected,
                                title,
                                hint,
                                &palette,
                                0.0,
                            );
                            if response.clicked() {
                                self.quick.selected = index;
                                chosen_command = command.clone();
                                chosen_file = path.clone();
                                close = true;
                            }
                        }
                    });
                if ui.input(|i| i.key_pressed(Key::Enter)) && !items.is_empty() {
                    if let Some((_, _, command, path)) =
                        items.get(self.quick.selected)
                    {
                        chosen_command = command.clone();
                        chosen_file = path.clone();
                        close = true;
                    }
                }
            });
        if modal.backdrop_response.clicked() || ctx.input(|i| i.key_pressed(Key::Escape)) {
            close = true;
        }
        if close {
            self.quick.open = false;
        }
        if let Some(command) = chosen_command {
            self.pending_command = Some(command);
        }
        if let Some(path) = chosen_file {
            self.pending_open = Some((path, None));
        }
    }

    fn show_settings(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        if self.settings_snapshot.is_none() {
            self.settings_snapshot = Some(self.settings.clone());
        }
        let palette = self.palette.clone();
        let lang = self.lang();
        let mut close = false;
        let mut save = false;
        let mut cancel = false;
        let mut reset = false;
        let mut restyle = false;
        let modal = Modal::new(Id::new("kodemo-settings-dialog"))
            .frame(modal_frame(&palette))
            .backdrop_color(Color32::from_black_alpha(110))
            .show(ctx, |ui| {
                ui.set_width(690.0);
                ui.horizontal(|ui| {
                    icons::draw(
                        ui.painter(),
                        egui::Rect::from_center_size(
                            ui.cursor().min + Vec2::new(10.0, 11.0),
                            Vec2::splat(18.0),
                        ),
                        Icon::Settings,
                        palette.accent,
                        1.25,
                    );
                    ui.add_space(26.0);
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(t(lang, "Settings", "Configurações"))
                                .size(15.0)
                                .strong()
                                .color(palette.fg),
                        );
                        ui.label(
                            RichText::new(t(
                                lang,
                                "Tune the editor, appearance, AI and preview.",
                                "Ajuste o editor, a aparência, a IA e a prévia.",
                            ))
                            .size(10.8)
                            .color(palette.fg_faint),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if small_icon_button(ui, Icon::Close, &palette, t(lang, "Close", "Fechar")).clicked() {
                            cancel = true;
                        }
                    });
                });
                ui.add_space(6.0);
                ScrollArea::vertical()
                    .id_salt("kodemo-settings-scroll")
                    .max_height(470.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        settings_section(ui, t(lang, "Appearance", "Aparência"), &palette);
                        settings_row(ui, t(lang, "Theme", "Tema"), &palette, |ui| {
                            ComboBox::from_id_salt("kodemo-settings-theme")
                                .selected_text(theme_name(lang, &self.settings.theme))
                                .width(230.0)
                                .show_ui(ui, |ui| {
                                    for name in theme::theme_names() {
                                        if ui
                                            .selectable_label(
                                                self.settings.theme == *name,
                                                theme_name(lang, name),
                                            )
                                            .clicked()
                                        {
                                            self.settings.theme = (*name).to_owned();
                                            if *name == "Custom"
                                                && self.settings.custom_theme.is_none()
                                            {
                                                self.settings.custom_theme =
                                                    Some(self.palette.clone());
                                            }
                                            restyle = true;
                                        }
                                    }
                                });
                        });
                        settings_row(ui, t(lang, "Language", "Idioma"), &palette, |ui| {
                            for value in [Lang::En, Lang::Pt] {
                                if ui
                                    .selectable_label(
                                        self.settings.lang == value,
                                        value.label(),
                                    )
                                    .clicked()
                                {
                                    self.settings.lang = value;
                                    self.settings_dirty = true;
                                }
                            }
                        });
                        settings_row(ui, t(lang, "Font size", "Tamanho da fonte"), &palette, |ui| {
                            if ui
                                .add(
                                    egui::Slider::new(&mut self.settings.font_size, 11.0..=24.0)
                                        .step_by(0.5)
                                        .suffix(" pt"),
                                )
                                .changed()
                            {
                                restyle = true;
                            }
                        });
                        settings_row(ui, t(lang, "Palette", "Paleta"), &palette, |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(t(lang, "Capture current colors", "Capturar cores atuais"))
                                            .size(10.5),
                                    )
                                    .fill(palette.panel_alt)
                                    .corner_radius(4.0),
                                )
                                .clicked()
                            {
                                self.settings.custom_theme = Some(self.palette.clone());
                                self.settings.theme = "Custom".to_owned();
                                restyle = true;
                            }
                        });
                        settings_row(ui, t(lang, "Line numbers", "Números de linha"), &palette, |ui| {
                            ui.checkbox(&mut self.settings.line_numbers, "");
                        });
                        settings_row(ui, t(lang, "Word wrap", "Quebra de linha"), &palette, |ui| {
                            if ui.checkbox(&mut self.settings.word_wrap, "").changed() {
                                self.highlight_cache = None;
                            }
                        });
                        settings_row(ui, t(lang, "Minimap", "Minimapa"), &palette, |ui| {
                            ui.checkbox(&mut self.settings.minimap, "");
                        });
                        settings_row(ui, t(lang, "Spell check", "Correção ortográfica"), &palette, |ui| {
                            if ui.checkbox(&mut self.settings.spellcheck, "").changed() {
                                self.spell_cache = None;
                            }
                        });
                        settings_row(ui, t(lang, "Word suggestions", "Sugestões de palavras"), &palette, |ui| {
                            if ui.checkbox(&mut self.settings.auto_complete, "").changed() {
                                self.completion.close();
                            }
                        });
                        settings_row(ui, t(lang, "Hover previews", "Prévias ao passar o mouse"), &palette, |ui| {
                            ui.checkbox(&mut self.settings.hover_previews, "");
                        });
                        settings_row(ui, t(lang, "Hover audio", "Áudio ao passar o mouse"), &palette, |ui| {
                            ui.add_enabled_ui(self.settings.hover_previews, |ui| {
                                ui.checkbox(&mut self.settings.hover_audio, "");
                            });
                        });
                        settings_row(ui, t(lang, "Animations", "Animações"), &palette, |ui| {
                            if ui.checkbox(&mut self.settings.animations, "").changed() {
                                restyle = true;
                            }
                        });

                        settings_section(ui, t(lang, "Editor", "Editor"), &palette);
                        settings_row(ui, t(lang, "Tab size", "Tamanho do Tab"), &palette, |ui| {
                            ui.add(egui::Slider::new(&mut self.settings.tab_size, 2..=8));
                        });
                        settings_row(ui, t(lang, "Auto save", "Salvar automaticamente"), &palette, |ui| {
                            ui.checkbox(&mut self.settings.auto_save, "");
                        });
                        settings_row(ui, t(lang, "Ghost suggestions", "Sugestões fantasma"), &palette, |ui| {
                            if ui.checkbox(&mut self.settings.ghost, "").changed() {
                                self.invalidate_ai();
                            }
                        });
                        settings_row(ui, t(lang, "Completion delay", "Atraso da conclusão"), &palette, |ui| {
                            ui.add(
                                egui::Slider::new(
                                    &mut self.settings.completion_delay_ms,
                                    150..=1500,
                                )
                                .suffix(" ms"),
                            );
                        });

                        settings_section(ui, t(lang, "Local AI", "IA local"), &palette);
                        settings_row(ui, t(lang, "Enabled", "Ativada"), &palette, |ui| {
                            if ui.checkbox(&mut self.settings.ai_enabled, "").changed() {
                                self.ai.configure(&self.settings);
                            }
                        });
                        settings_row(ui, t(lang, "Endpoint", "Endpoint"), &palette, |ui| {
                            ui.add(
                                TextEdit::singleline(&mut self.settings.ai_endpoint)
                                    .font(FontId::monospace(10.8))
                                    .desired_width(330.0)
                                    .margin(Margin::symmetric(6, 3)),
                            );
                        });
                        settings_row(ui, t(lang, "Model", "Modelo"), &palette, |ui| {
                            ui.add(
                                TextEdit::singleline(&mut self.settings.ai_model)
                                    .font(FontId::monospace(10.8))
                                    .desired_width(330.0)
                                    .margin(Margin::symmetric(6, 3)),
                            );
                        });
                        settings_row(ui, t(lang, "llama-server path", "Caminho do llama-server"), &palette, |ui| {
                            ui.add(
                                TextEdit::singleline(&mut self.settings.ai_server_path)
                                    .font(FontId::monospace(10.8))
                                    .desired_width(330.0)
                                    .hint_text(t(lang, "Auto-discover", "Detectar automaticamente"))
                                    .margin(Margin::symmetric(6, 3)),
                            );
                        });
                        settings_row(ui, t(lang, "GGUF model path", "Caminho do modelo GGUF"), &palette, |ui| {
                            ui.add(
                                TextEdit::singleline(&mut self.settings.ai_model_path)
                                    .font(FontId::monospace(10.8))
                                    .desired_width(330.0)
                                    .hint_text(t(lang, "Auto-discover", "Detectar automaticamente"))
                                    .margin(Margin::symmetric(6, 3)),
                            );
                        });
                        settings_row(ui, t(lang, "Start server automatically", "Iniciar servidor automaticamente"), &palette, |ui| {
                            ui.checkbox(&mut self.settings.ai_auto_start, "");
                        });
                        settings_row(ui, t(lang, "Max suggestion tokens", "Máximo de tokens da sugestão"), &palette, |ui| {
                            ui.add(egui::Slider::new(&mut self.settings.ai_max_tokens, 8..=128));
                        });

                        settings_section(ui, t(lang, "Live preview", "Prévia ao vivo"), &palette);
                        settings_row(ui, t(lang, "HTTP port", "Porta HTTP"), &palette, |ui| {
                            ui.add(
                                egui::DragValue::new(&mut self.settings.preview_port)
                                    .range(1024..=65535)
                                    .speed(1.0),
                            );
                        });
                        settings_row(ui, t(lang, "Open on launch", "Abrir ao iniciar"), &palette, |ui| {
                            ui.checkbox(&mut self.settings.preview_auto_start, "");
                        });
                        settings_row(ui, t(lang, "Live reload", "Recarregamento ao vivo"), &palette, |ui| {
                            ui.checkbox(&mut self.settings.preview_live_reload, "");
                        });

                        settings_section(ui, t(lang, "Files and rendering", "Arquivos e renderização"), &palette);
                        settings_row(ui, t(lang, "Show hidden files", "Mostrar arquivos ocultos"), &palette, |ui| {
                            if ui.checkbox(&mut self.settings.show_hidden, "").changed() {
                                self.explorer.set_show_hidden(self.settings.show_hidden);
                            }
                        });
                        settings_row(ui, t(lang, "Force software rendering", "Forçar renderização por software"), &palette, |ui| {
                            ui.checkbox(&mut self.settings.software_rendering, "");
                            ui.label(
                                RichText::new(t(lang, "Restart required", "Requer reinício"))
                                    .size(9.8)
                                    .color(palette.fg_faint),
                            );
                        });

                        settings_section(ui, t(lang, "About this configuration", "Sobre esta configuração"), &palette);
                        ui.label(
                            RichText::new(format!(
                                "{}: {}",
                                t(lang, "Settings file", "Arquivo de configuração"),
                                Settings::path().display()
                            ))
                            .size(10.0)
                            .color(palette.fg_faint),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{}: {}",
                                t(lang, "Rendering", "Renderização"),
                                if self.software {
                                    t(lang, "software (OpenGL fallback)", "software (fallback OpenGL)")
                                } else {
                                    t(lang, "hardware accelerated", "acelerada por hardware")
                                }
                            ))
                            .size(10.0)
                            .color(palette.fg_faint),
                        );
                        ui.add_space(8.0);
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(t(lang, "Restore defaults", "Restaurar padrões"))
                                    .size(10.5)
                                    .color(palette.fg_muted),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        reset = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(t(lang, "Save", "Salvar"))
                                        .color(palette.accent_fg),
                                )
                                .fill(palette.accent)
                                .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            save = true;
                            close = true;
                        }
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(t(lang, "Cancel", "Cancelar"))
                                        .color(palette.fg),
                                )
                                .fill(palette.panel_alt)
                                .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            cancel = true;
                            close = true;
                        }
                    });
                });
            });
        let _ = modal;
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            cancel = true;
            close = true;
        }
        if reset {
            let mut defaults = Settings::default();
            defaults.lang = self.settings.lang;
            defaults.last_workspace = self.settings.last_workspace.clone();
            defaults.recent_files = self.settings.recent_files.clone();
            defaults.recent_folders = self.settings.recent_folders.clone();
            defaults.theme = "Kodemo Dark".to_owned();
            self.settings = defaults;
            self.settings_dirty = true;
            restyle = true;
        }
        if restyle {
            self.apply_theme(ctx);
        }
        if cancel {
            if let Some(snapshot) = self.settings_snapshot.take() {
                self.settings = snapshot;
                self.explorer.set_show_hidden(self.settings.show_hidden);
                self.ai.configure(&self.settings);
                self.apply_theme(ctx);
            }
            self.settings_dirty = false;
            self.settings_open = false;
        } else if save {
            self.settings_snapshot = None;
            self.settings_open = false;
            self.settings.normalize();
            match self.settings.save() {
                Ok(()) => self.set_status(
                    t(lang, "Settings saved", "Configurações salvas"),
                    StatusKind::Success,
                ),
                Err(err) => self.set_status(err, StatusKind::Error),
            }
            self.explorer.set_show_hidden(self.settings.show_hidden);
            self.ai.configure(&self.settings);
            self.apply_theme(ctx);
            self.settings_dirty = false;
        } else if close {
            self.settings_open = false;
        }
    }

    fn show_about(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        let palette = self.palette.clone();
        let lang = self.lang();
        let mut close = false;
        let mut help = false;
        let modal = Modal::new(Id::new("kodemo-about-dialog"))
            .frame(modal_frame(&palette))
            .backdrop_color(Color32::from_black_alpha(110))
            .show(ctx, |ui| {
                ui.set_width(470.0);
                if let Some(banner) = self.banner.clone() {
                    // Centered, spanning the dialog width. The aspect ratio
                    // comes from the texture so the banner is never stretched.
                    let size = banner.size_vec2();
                    let width = ui.available_width();
                    let height = if size.x > 0.0 {
                        width * size.y / size.x
                    } else {
                        width / 3.0
                    };
                    ui.vertical_centered(|ui| {
                        ui.add(
                            egui::Image::from_texture(&banner)
                                .fit_to_exact_size(Vec2::new(width, height)),
                        );
                    });
                    ui.add_space(6.0);
                }
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} {} · {}",
                            t(lang, "version", "versão"),
                            env!("CARGO_PKG_VERSION"),
                            if self.software {
                                t(lang, "software rendering", "renderização por software")
                            } else {
                                t(lang, "hardware accelerated", "acelerada por hardware")
                            }
                        ))
                        .size(10.4)
                        .color(palette.fg_faint),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(t(
                            lang,
                            "A native IDE for modest machines. Rust, egui, WebKitGTK and a local Qwen model — no Electron, no telemetry.",
                            "Uma IDE nativa para máquinas modestas. Rust, egui, WebKitGTK e um modelo Qwen local — sem Electron, sem telemetria.",
                        ))
                        .size(10.8)
                        .color(palette.fg_muted),
                    );
                    ui.add_space(7.0);
                    ui.horizontal(|ui| {
                        ui.add_space(((ui.available_width() - 300.0) * 0.5).max(0.0));
                        for label in ["Rust", "egui 0.31", "WebKitGTK", "Qwen 0.5B"] {
                            Frame::NONE
                                .fill(palette.panel_alt)
                                .corner_radius(10.0)
                                .inner_margin(Margin::symmetric(8, 3))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(label)
                                            .size(9.5)
                                            .color(palette.fg_muted),
                                    );
                                });
                        }
                    });
                });
                ui.add_space(8.0);
                let mut specs = vec![
                    (
                        t(lang, "Processor", "Processador").to_owned(),
                        self.hardware.cpu.clone(),
                    ),
                    (
                        t(lang, "Cores", "Núcleos").to_owned(),
                        format!(
                            "{} {}",
                            self.hardware.cores,
                            t(lang, "threads", "threads")
                        ),
                    ),
                    (
                        t(lang, "Memory", "Memória").to_owned(),
                        self.hardware.memory.clone(),
                    ),
                    (
                        t(lang, "System", "Sistema").to_owned(),
                        self.hardware.os.clone(),
                    ),
                    ("Kernel".to_owned(), self.hardware.kernel.clone()),
                    (
                        t(lang, "Architecture", "Arquitetura").to_owned(),
                        self.hardware.architecture.clone(),
                    ),
                ];
                if !self.hardware.device.is_empty() {
                    specs.push((
                        t(lang, "Device", "Dispositivo").to_owned(),
                        self.hardware.device.clone(),
                    ));
                }
                Frame::NONE
                    .fill(palette.panel_alt)
                    .corner_radius(8.0)
                    .inner_margin(Margin::symmetric(12, 9))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(t(lang, "THIS DEVICE", "ESTE DISPOSITIVO"))
                                .size(9.4)
                                .strong()
                                .color(palette.fg_faint),
                        );
                        ui.add_space(3.0);
                        egui::Grid::new("kodemo-about-hardware")
                            .num_columns(2)
                            .spacing([14.0, 3.0])
                            .show(ui, |ui| {
                                for (label, value) in &specs {
                                    ui.label(
                                        RichText::new(label)
                                            .size(10.2)
                                            .color(palette.fg_faint),
                                    );
                                    ui.label(
                                        RichText::new(util::trimmed_center(value, 58))
                                            .size(10.4)
                                            .color(palette.fg_muted),
                                    )
                                    .on_hover_text(value);
                                    ui.end_row();
                                }
                            });
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}: {}",
                            t(lang, "Config", "Configuração"),
                            Settings::path().display()
                        ))
                        .size(9.5)
                        .color(palette.fg_faint),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(t(lang, "Close", "Fechar"))
                                        .color(palette.accent_fg),
                                )
                                .fill(palette.accent)
                                .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            close = true;
                        }
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(t(lang, "Keyboard shortcuts", "Atalhos de teclado"))
                                        .size(10.5),
                                )
                                .fill(palette.panel_alt)
                                .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            help = true;
                        }
                    });
                });
            });
        if modal.backdrop_response.clicked() || ctx.input(|i| i.key_pressed(Key::Escape)) {
            close = true;
        }
        if help {
            self.about_open = false;
            self.help_open = true;
        }
        if close {
            self.about_open = false;
        }
    }

    fn show_help(&mut self, ctx: &egui::Context) {
        if !self.help_open {
            return;
        }
        let palette = self.palette.clone();
        let lang = self.lang();
        let mut close = false;
        let entries = [
            (
                t(lang, "Command palette", "Paleta de comandos"),
                "Ctrl+Shift+P",
            ),
            (t(lang, "Quick open file", "Abrir arquivo rápido"), "Ctrl+P"),
            (t(lang, "Save", "Salvar"), "Ctrl+S"),
            (t(lang, "Save as", "Salvar como"), "Ctrl+Shift+S"),
            (t(lang, "Close editor", "Fechar editor"), "Ctrl+W"),
            (
                t(lang, "Toggle sidebar", "Alternar barra lateral"),
                "Ctrl+B",
            ),
            (t(lang, "Toggle panel", "Alternar painel"), "Ctrl+J"),
            (
                t(lang, "Toggle live preview", "Alternar prévia ao vivo"),
                "Ctrl+Alt+P",
            ),
            (t(lang, "Settings", "Configurações"), "Ctrl+,"),
            (t(lang, "Find in files", "Localizar nos arquivos"), "Ctrl+Shift+F"),
            (t(lang, "Go to line", "Ir para linha"), "Ctrl+G"),
            (t(lang, "Terminal", "Terminal"), "Ctrl+`"),
            (
                t(lang, "Zoom in / out", "Aumentar / diminuir zoom"),
                "Ctrl+= / Ctrl+-",
            ),
            (
                t(lang, "Full screen", "Tela cheia"),
                "F11",
            ),
            (
                t(lang, "Accept suggestion", "Aceitar sugestão"),
                "Tab",
            ),
        ];
        let modal = Modal::new(Id::new("kodemo-help-dialog"))
            .frame(modal_frame(&palette))
            .backdrop_color(Color32::from_black_alpha(110))
            .show(ctx, |ui| {
                ui.set_width(560.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(t(lang, "Keyboard shortcuts", "Atalhos de teclado"))
                            .size(15.0)
                            .strong()
                            .color(palette.fg),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if small_icon_button(ui, Icon::Close, &palette, t(lang, "Close", "Fechar")).clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(8.0);
                ScrollArea::vertical()
                    .id_salt("kodemo-help-scroll")
                    .max_height(430.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Grid::new("kodemo-help-grid")
                            .num_columns(2)
                            .spacing([20.0, 7.0])
                            .striped(true)
                            .show(ui, |ui| {
                                for (action, shortcut) in entries {
                                    ui.label(
                                        RichText::new(action)
                                            .size(11.5)
                                            .color(palette.fg_muted),
                                    );
                                    ui.label(
                                        RichText::new(shortcut)
                                            .font(FontId::monospace(10.8))
                                            .color(palette.fg),
                                    );
                                    ui.end_row();
                                }
                            });
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(t(
                                lang,
                                "Tip: the command palette lists every action, even those without a shortcut.",
                                "Dica: a paleta de comandos lista todas as ações, mesmo as sem atalho.",
                            ))
                            .size(10.5)
                            .color(palette.fg_faint),
                        );
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(t(lang, "Close", "Fechar"))
                                        .color(palette.accent_fg),
                                )
                                .fill(palette.accent)
                                .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            close = true;
                        }
                    });
                });
            });
        if modal.backdrop_response.clicked() || ctx.input(|i| i.key_pressed(Key::Escape)) {
            close = true;
        }
        if close {
            self.help_open = false;
        }
    }
}

/// Sidebar padding grows gently with the panel width so narrow and wide
/// layouts both keep a consistent visual margin.
fn sidebar_pad(ui: &Ui) -> f32 {
    (ui.available_width() * 0.05).clamp(10.0, 18.0)
}

fn modal_frame(palette: &ThemePalette) -> Frame {
    Frame::NONE
        .fill(palette.panel)
        .stroke(Stroke::new(1.0_f32, palette.border_strong))
        .corner_radius(10.0)
        .inner_margin(Margin::symmetric(18, 15))
}

fn settings_section(ui: &mut Ui, title: &str, palette: &ThemePalette) {
    ui.add_space(8.0);
    ui.label(
        RichText::new(title.to_uppercase())
            .size(9.8)
            .strong()
            .color(palette.accent),
    );
    ui.add_space(2.0);
    ui.separator();
    ui.add_space(2.0);
}

fn settings_row(
    ui: &mut Ui,
    label: &str,
    palette: &ThemePalette,
    add: impl FnOnce(&mut Ui),
) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [170.0, 22.0],
            egui::Label::new(
                RichText::new(label)
                    .size(11.3)
                    .color(palette.fg_muted),
            ),
        );
        add(ui);
    });
    ui.add_space(1.0);
}

fn command_palette(lang: Lang) -> Vec<(String, Command, &'static str)> {
    vec![
        (t(lang, "New File…", "Novo arquivo…").to_owned(), Command::NewFile, "Ctrl+N"),
        (t(lang, "Open File…", "Abrir arquivo…").to_owned(), Command::OpenFile, "Ctrl+O"),
        (t(lang, "Open Folder…", "Abrir pasta…").to_owned(), Command::OpenFolder, "Ctrl+Shift+O"),
        (t(lang, "Save", "Salvar").to_owned(), Command::Save, "Ctrl+S"),
        (t(lang, "Save As…", "Salvar como…").to_owned(), Command::SaveAs, "Ctrl+Shift+S"),
        (t(lang, "Save All", "Salvar tudo").to_owned(), Command::SaveAll, ""),
        (t(lang, "Close Editor", "Fechar editor").to_owned(), Command::CloseTab, "Ctrl+W"),
        (t(lang, "Close Other Editors", "Fechar outros editores").to_owned(), Command::CloseOthers, ""),
        (t(lang, "Quick Open File", "Abrir arquivo rápido").to_owned(), Command::QuickOpen, "Ctrl+P"),
        (t(lang, "Command Palette", "Paleta de comandos").to_owned(), Command::CommandPalette, "Ctrl+Shift+P"),
        (t(lang, "Find in Files", "Localizar nos arquivos").to_owned(), Command::FindInFiles, "Ctrl+Shift+F"),
        (t(lang, "Go to Line…", "Ir para linha…").to_owned(), Command::GoToLine, "Ctrl+G"),
        (t(lang, "Toggle Line Comment", "Comentar linha").to_owned(), Command::ToggleComment, "Ctrl+/"),
        (t(lang, "Toggle Sidebar", "Alternar barra lateral").to_owned(), Command::ToggleSidebar, "Ctrl+B"),
        (t(lang, "Toggle Bottom Panel", "Alternar painel inferior").to_owned(), Command::TogglePanel, "Ctrl+J"),
        (t(lang, "Toggle Live Preview", "Alternar prévia ao vivo").to_owned(), Command::TogglePreview, "Ctrl+Alt+P"),
        (t(lang, "Toggle Word Wrap", "Alternar quebra de linha").to_owned(), Command::ToggleWordWrap, ""),
        (t(lang, "Toggle Minimap", "Alternar minimapa").to_owned(), Command::ToggleMinimap, ""),
        (t(lang, "Toggle Ghost Suggestions", "Alternar sugestões fantasma").to_owned(), Command::ToggleGhost, ""),
        (t(lang, "Run Project", "Executar projeto").to_owned(), Command::RunProject, "Ctrl+F5"),
        (t(lang, "Compile Project", "Compilar projeto").to_owned(), Command::CompileProject, "Ctrl+Shift+B"),
        (t(lang, "Close All Files and Project", "Fechar todos os arquivos e projeto").to_owned(), Command::CloseAll, ""),
        (t(lang, "Close Project", "Fechar projeto").to_owned(), Command::CloseProject, ""),
        (t(lang, "Toggle AI", "Alternar IA").to_owned(), Command::ToggleAi, ""),
        (t(lang, "Restart AI Server", "Reiniciar servidor de IA").to_owned(), Command::RestartAi, ""),
        (t(lang, "Reload Preview", "Recarregar prévia").to_owned(), Command::ReloadPreview, "F5"),
        (t(lang, "Open Preview in Browser", "Abrir prévia no navegador").to_owned(), Command::OpenInBrowser, ""),
        (t(lang, "Toggle Terminal", "Alternar terminal").to_owned(), Command::ToggleTerminal, "Ctrl+`"),
        (t(lang, "New Terminal", "Novo terminal").to_owned(), Command::NewTerminal, "Ctrl+Shift+`"),
        (t(lang, "Clear Terminal", "Limpar terminal").to_owned(), Command::ClearTerminal, ""),
        (t(lang, "Copy Path", "Copiar caminho").to_owned(), Command::CopyPath, ""),
        (t(lang, "Reveal in File Manager", "Mostrar no gerenciador de arquivos").to_owned(), Command::RevealActive, ""),
        (t(lang, "Settings", "Configurações").to_owned(), Command::OpenSettings, "Ctrl+,"),
        (t(lang, "Keyboard Shortcuts", "Atalhos de teclado").to_owned(), Command::OpenHelp, ""),
        (t(lang, "About Kodemo", "Sobre o Kodemo").to_owned(), Command::OpenAbout, ""),
        (t(lang, "Zoom In", "Aumentar zoom").to_owned(), Command::ZoomIn, "Ctrl+="),
        (t(lang, "Zoom Out", "Diminuir zoom").to_owned(), Command::ZoomOut, "Ctrl+-"),
        (t(lang, "Reset Zoom", "Redefinir zoom").to_owned(), Command::ZoomReset, "Ctrl+0"),
        (t(lang, "Toggle Full Screen", "Alternar tela cheia").to_owned(), Command::ToggleFullscreen, "F11"),
        (t(lang, "Exit", "Sair").to_owned(), Command::Exit, ""),
    ]
}

fn welcome_action(ui: &mut Ui, icon: Icon, label: &str, palette: &ThemePalette) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(118.0, 68.0), Sense::click());
    let fill = if response.hovered() {
        palette.panel_alt
    } else {
        palette.panel
    };
    ui.painter().rect_filled(rect, 8.0, fill);
    ui.painter().rect_stroke(
        rect,
        8.0,
        Stroke::new(1.0_f32, palette.border),
        StrokeKind::Inside,
    );
    icons::draw(
        ui.painter(),
        egui::Rect::from_center_size(
            Pos2::new(rect.center().x, rect.top() + 24.0),
            Vec2::splat(26.0),
        ),
        icon,
        palette.accent,
        1.2,
    );
    ui.painter().text(
        Pos2::new(rect.center().x, rect.bottom() - 14.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(10.6),
        palette.fg_muted,
    );
    response.clicked()
}

fn language_color(language: Language, palette: &ThemePalette) -> Color32 {
    match language {
        Language::Rust => palette.syntax_tag,
        Language::Python => palette.warning,
        Language::JavaScript | Language::TypeScript => palette.syntax_attribute,
        Language::Html => palette.syntax_keyword,
        Language::Css => palette.syntax_property,
        Language::Json => palette.warning,
        Language::Markdown => palette.accent,
        Language::Toml | Language::Yaml => palette.syntax_type,
        Language::Shell => palette.success,
        Language::Sql => palette.info,
        Language::C | Language::Cpp => palette.syntax_function,
        Language::Php => palette.syntax_type,
        Language::Java => palette.error,
        Language::Go => palette.info,
        Language::Plain => palette.fg_faint,
    }
}

fn draw_spell_underlines(
    ui: &Ui,
    galley: &egui::Galley,
    galley_pos: Pos2,
    clip: egui::Rect,
    issues: &[spell::Issue],
    cursor: usize,
    color: Color32,
) {
    if issues.is_empty() {
        return;
    }
    let painter = ui.painter().with_clip_rect(clip);
    let mut char_index = 0usize;
    let mut issue_index = 0usize;
    for row in &galley.rows {
        let row_start = char_index;
        let row_end = row_start + row.glyphs.len();
        while issue_index < issues.len() && issues[issue_index].end <= row_start {
            issue_index += 1;
        }
        let y = galley_pos.y + row.rect.bottom() - 2.0;
        let visible = y >= clip.top() - 2.0 && galley_pos.y + row.rect.top() <= clip.bottom() + 2.0;
        if visible && row_end > row_start {
            let mut index = issue_index;
            while index < issues.len() && issues[index].start < row_end {
                let issue = issues[index];
                if !(issue.start <= cursor && cursor <= issue.end) {
                    let start = issue.start.max(row_start) - row_start;
                    let end = issue.end.min(row_end) - row_start;
                    if end > start {
                        let x1 = galley_pos.x + row.x_offset(start);
                        let x2 = galley_pos.x + row.x_offset(end);
                        if x2 > x1 {
                            let mut points = Vec::with_capacity(((x2 - x1) / 2.5) as usize + 2);
                            let mut x = x1;
                            let mut high = false;
                            while x < x2 {
                                points.push(Pos2::new(x, y + if high { -0.8 } else { 0.8 }));
                                high = !high;
                                x += 2.5;
                            }
                            points.push(Pos2::new(x2, y));
                            painter.add(egui::Shape::line(points, Stroke::new(1.0_f32, color)));
                        }
                    }
                }
                index += 1;
            }
        }
        char_index = row_end + row.ends_with_newline as usize;
    }
}

fn draw_minimap(ui: &mut Ui, rect: egui::Rect, text: &str, palette: &ThemePalette) {
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, palette.panel);
    painter.vline(
        rect.left() + 0.5,
        rect.y_range(),
        Stroke::new(1.0_f32, palette.border),
    );
    let lines: Vec<&str> = text.lines().take(240).collect();
    if lines.is_empty() {
        return;
    }
    let row = (rect.height() / lines.len() as f32).clamp(0.8, 2.6);
    let mut y = rect.top() + 4.0;
    for line in &lines {
        let trimmed = line.trim_end();
        if !trimmed.trim().is_empty() {
            let indent = line.len() - line.trim_start().len();
            let x = rect.left() + 5.0 + (indent as f32 * 0.55).min(rect.width() * 0.32);
            let width = ((trimmed.len() as f32 * 0.9).min(rect.width() - 9.0)).max(2.0);
            let color = if trimmed.trim_start().starts_with("//")
                || trimmed.trim_start().starts_with('#')
            {
                theme::alpha(palette.syntax_comment, 150)
            } else {
                theme::alpha(palette.fg_faint, 125)
            };
            painter.rect_filled(
                egui::Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, row.max(1.0))),
                0.5,
                color,
            );
        }
        y += row + 0.7;
        if y > rect.bottom() - 2.0 {
            break;
        }
    }
}

fn activity_button(
    ui: &mut Ui,
    icon: Icon,
    active: bool,
    palette: &ThemePalette,
    tooltip: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(46.0, 42.0), Sense::click());
    let pill = rect.shrink2(Vec2::new(4.0, 3.0));
    if active {
        ui.painter()
            .rect_filled(pill, 8.0, theme::alpha(palette.accent, 26));
        ui.painter().rect_filled(
            egui::Rect::from_center_size(
                Pos2::new(rect.left() + 3.0, rect.center().y),
                Vec2::new(3.0, 20.0),
            ),
            1.5,
            palette.accent,
        );
    } else if response.hovered() {
        ui.painter().rect_filled(pill, 8.0, palette.hover);
    }
    icons::draw(
        ui.painter(),
        egui::Rect::from_center_size(rect.center(), Vec2::splat(20.0)),
        icon,
        if active {
            palette.accent
        } else {
            palette.fg_muted
        },
        1.25,
    );
    response.on_hover_text(tooltip)
}

fn inline_icon(ui: &mut Ui, icon: Icon, size: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    icons::draw(
        ui.painter(),
        egui::Rect::from_center_size(rect.center(), Vec2::splat(size * 0.78)),
        icon,
        color,
        1.1,
    );
}

fn inline_icon_button(
    ui: &mut Ui,
    icon: Icon,
    size: f32,
    color: Color32,
    tooltip: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 3.0, theme::alpha(color, 38));
    }
    icons::draw(
        ui.painter(),
        egui::Rect::from_center_size(rect.center(), Vec2::splat(size * 0.72)),
        icon,
        color,
        1.1,
    );
    response.on_hover_text(tooltip)
}

fn icon_button(
    ui: &mut Ui,
    icon: Icon,
    size: f32,
    palette: &ThemePalette,
    tooltip: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 4.0, palette.hover);
    }
    icons::draw(
        ui.painter(),
        egui::Rect::from_center_size(rect.center(), Vec2::splat(size * 0.68)),
        icon,
        palette.fg_muted,
        1.1,
    );
    response.on_hover_text(tooltip)
}

fn small_icon_button(
    ui: &mut Ui,
    icon: Icon,
    palette: &ThemePalette,
    tooltip: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 4.0, palette.hover);
    }
    icons::draw(
        ui.painter(),
        egui::Rect::from_center_size(rect.center(), Vec2::splat(13.0)),
        icon,
        palette.fg_muted,
        1.15,
    );
    response.on_hover_text(tooltip)
}

fn result_row(
    ui: &mut Ui,
    selected: bool,
    title: &str,
    hint: &str,
    palette: &ThemePalette,
    indent: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 23.0),
        Sense::click(),
    );
    if selected {
        ui.painter()
            .rect_filled(rect, 2.0, theme::alpha(palette.accent, 38));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, palette.hover);
    }
    ui.painter().text(
        Pos2::new(rect.left() + 10.0 + indent, rect.center().y),
        Align2::LEFT_CENTER,
        util::trimmed_center(title, 72),
        FontId::monospace(10.8),
        palette.fg_muted,
    );
    if !hint.is_empty() {
        ui.painter().text(
            Pos2::new(rect.right() - 8.0, rect.center().y),
            Align2::RIGHT_CENTER,
            hint,
            FontId::monospace(9.8),
            palette.fg_faint,
        );
    }
    response
}

fn editor_id(doc_id: u64) -> Id {
    Id::new("kodemo-editor").with(doc_id)
}

fn menu_item(
    ui: &mut Ui,
    label: &str,
    shortcut: &str,
    command: Command,
    out: &mut Option<Command>,
) {
    let button = egui::Button::new(label)
        .shortcut_text(shortcut)
        .min_size(Vec2::new(220.0, 0.0))
        .frame(false);
    if ui.add(button).clicked() {
        *out = Some(command);
        ui.close_menu();
    }
}

fn menu_toggle(
    ui: &mut Ui,
    label: &str,
    checked: bool,
    shortcut: &str,
    command: Command,
    out: &mut Option<Command>,
) {
    let text = format!("{} {}", if checked { "●" } else { "○" }, label);
    menu_item(ui, &text, shortcut, command, out);
}

fn title_run_button(
    ui: &mut Ui,
    palette: &ThemePalette,
    label: &str,
    tooltip: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(76.0, 26.0), Sense::click());
    let fill = if response.hovered() {
        palette.accent
    } else {
        theme::alpha(palette.accent, 220)
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    icons::draw(
        ui.painter(),
        egui::Rect::from_center_size(
            Pos2::new(rect.left() + 14.0, rect.center().y),
            Vec2::splat(13.0),
        ),
        Icon::Play,
        palette.accent_fg,
        1.2,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 24.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.5),
        palette.accent_fg,
    );
    response.on_hover_text(tooltip)
}

fn title_icon_button(
    ui: &mut Ui,
    icon: Icon,
    active: bool,
    palette: &ThemePalette,
    tooltip: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(28.0, 26.0), Sense::click());
    if active {
        ui.painter().rect_filled(rect, 6.0, theme::alpha(palette.accent, 34));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 6.0, palette.hover);
    }
    icons::draw(
        ui.painter(),
        egui::Rect::from_center_size(rect.center(), Vec2::splat(15.0)),
        icon,
        if active { palette.fg } else { palette.fg_muted },
        1.2,
    );
    response.on_hover_text(tooltip)
}

/// Battery-shaped toggle for Energy Saver. Drawn by hand so the glyph is
/// guaranteed to exist regardless of the icon font version.
fn energy_saver_button(ui: &mut Ui, active: bool, palette: &ThemePalette, tooltip: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(28.0, 26.0), Sense::click());
    if active {
        ui.painter()
            .rect_filled(rect, 6.0, theme::alpha(palette.accent, 46));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 6.0, palette.hover);
    }
    let color = if active { palette.accent } else { palette.fg_muted };
    let body = egui::Rect::from_center_size(rect.center() - Vec2::new(2.0, 0.0), Vec2::new(15.0, 9.0));
    let painter = ui.painter();
    painter.rect_stroke(
        body,
        2.0,
        Stroke::new(1.3_f32, color),
        StrokeKind::Inside,
    );
    let nub = egui::Rect::from_min_size(
        Pos2::new(body.right() + 1.0, body.center().y - 2.5),
        Vec2::new(2.2, 5.0),
    );
    painter.rect_filled(nub, 1.0, color);
    if active {
        let fill = body.shrink(2.3);
        painter.rect_filled(fill, 1.0, color);
    }
    response.on_hover_text(tooltip)
}

/// The start-page logo. `assets/kodemo-icon.png` is rasterized from the
/// canonical `assets/icon.svg` at 1024 px, the highest resolution egui's
/// texture path can use, so it stays sharp on HiDPI displays.
fn load_logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let image = image::load_from_memory(include_bytes!("../assets/kodemo-icon.png")).ok()?;
    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture("kodemo-icon", color, egui::TextureOptions::LINEAR))
}

fn load_banner(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let image = image::load_from_memory(include_bytes!("../assets/banner.png")).ok()?;
    // Some modest GPUs (including the Intel HD this app targets) cap textures
    // at 2048 px, and the source banner is 2400 px wide.
    let max_side = 1200.0;
    let width = image.width() as f32;
    let height = image.height() as f32;
    let scale = (max_side / width).min(max_side / height).min(1.0);
    let image = if scale < 1.0 {
        image.resize(
            (width * scale).round() as u32,
            (height * scale).round() as u32,
            image::imageops::FilterType::Triangle,
        )
    } else {
        image
    };
    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture("kodemo-banner", color, egui::TextureOptions::LINEAR))
}

/// Closing brackets, quotes and HTML tags typed by the user.
const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
    "source", "track", "wbr",
];

/// Pair a single character the user just typed with its closing character.
///
/// `previous` is the document from before this frame and `cursor` is the
/// caret after the insertion. Returns the caret position to apply when a
/// pairing (or skipping over an existing closer) happened.
fn auto_close_edit(
    text: &mut String,
    previous: &str,
    cursor: usize,
    language: Language,
) -> Option<usize> {
    if cursor == 0 || text.chars().count() != previous.chars().count() + 1 {
        return None;
    }
    let inserted_byte = util::char_to_byte(text, cursor - 1);
    let inserted = text[inserted_byte..].chars().next()?;
    let after_byte = util::char_to_byte(text, cursor);
    let previous_split = util::char_to_byte(previous, cursor - 1);
    // Only act on exactly one inserted character at the caret.
    if text[..inserted_byte] != previous[..previous_split]
        || text[after_byte..] != previous[previous_split..]
    {
        return None;
    }
    let next_char = text[after_byte..].chars().next();
    let prev_char = text[..inserted_byte].chars().next_back();
    match inserted {
        '(' | '[' | '{' => {
            let closer = match inserted {
                '(' => ')',
                '[' => ']',
                _ => '}',
            };
            // Don't double a closer that is already there, and don't wrap a
            // word the user is typing in the middle of.
            if next_char == Some(closer)
                || next_char.is_some_and(|c| c.is_alphanumeric())
            {
                return None;
            }
            text.insert(after_byte, closer);
            Some(cursor)
        }
        ')' | ']' | '}' => {
            // Typing the closer that is already there just steps over it.
            if next_char == Some(inserted) {
                text.replace_range(inserted_byte..after_byte, "");
                Some(cursor - 1)
            } else {
                None
            }
        }
        '"' | '\'' | '`' => {
            if next_char == Some(inserted) {
                text.replace_range(inserted_byte..after_byte, "");
                return Some(cursor - 1);
            }
            // Don't turn an apostrophe in prose into a pair, and don't wrap a
            // quote around the middle of a word.
            if next_char.is_some_and(|c| c.is_alphanumeric()) {
                return None;
            }
            if inserted == '\'' && prev_char.is_some_and(|c| c.is_alphanumeric()) {
                return None;
            }
            text.insert(after_byte, inserted);
            Some(cursor)
        }
        '>' if language == Language::Html => {
            if next_char == Some('<') {
                return None;
            }
            let before = &text[..inserted_byte];
            let line = before.rsplit('\n').next().unwrap_or_default();
            let open = line.rfind('<')?;
            let tag_text = &line[open + 1..];
            if tag_text.starts_with('/') || tag_text.starts_with('!') || tag_text.starts_with('?') {
                return None;
            }
            let name_end = tag_text
                .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
                .unwrap_or(tag_text.len());
            let name = &tag_text[..name_end];
            if name.is_empty()
                || !name.chars().next()?.is_ascii_alphabetic()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == ':' || c == '_')
            {
                return None;
            }
            if VOID_TAGS.iter().any(|tag| tag.eq_ignore_ascii_case(name)) {
                return None;
            }
            let close = format!("</{name}>");
            if text[after_byte..].starts_with(&close) {
                return None;
            }
            text.insert_str(after_byte, &close);
            Some(cursor)
        }
        _ => None,
    }
}

fn line_start_char(text: &str, line_index: usize) -> usize {
    let mut line = 0;
    for (char_index, ch) in text.chars().enumerate() {
        if line == line_index {
            return char_index;
        }
        if ch == '\n' {
            line += 1;
        }
    }
    text.chars().count()
}

fn encode_path(path: &Path) -> String {
    let mut out = String::new();
    for component in path.components() {
        let text = component.as_os_str().to_string_lossy();
        if !out.is_empty() {
            out.push('/');
        }
        for byte in text.as_bytes() {
            let ch = *byte as char;
            if ch.is_ascii_alphanumeric() || "-._~".contains(ch) {
                out.push(ch);
            } else {
                out.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod auto_close_tests {
    use super::auto_close_edit;
    use crate::syntax::Language;

    #[test]
    fn pairs_brackets_and_quotes() {
        let mut text = "(".to_owned();
        assert_eq!(auto_close_edit(&mut text, "", 1, Language::Rust), Some(1));
        assert_eq!(text, "()");
        let mut text = "\"".to_owned();
        assert_eq!(auto_close_edit(&mut text, "", 1, Language::Rust), Some(1));
        assert_eq!(text, "\"\"");
    }

    #[test]
    fn steps_over_an_existing_closer() {
        let mut text = "())".to_owned();
        assert_eq!(auto_close_edit(&mut text, "()", 2, Language::Rust), Some(1));
        assert_eq!(text, "()");
    }

    #[test]
    fn closes_html_tags() {
        let mut text = "<div>".to_owned();
        assert_eq!(auto_close_edit(&mut text, "<div", 5, Language::Html), Some(5));
        assert_eq!(text, "<div></div>");
        let mut text = "<br>".to_owned();
        assert_eq!(auto_close_edit(&mut text, "<br", 4, Language::Html), None);
    }

    #[test]
    fn ignores_paste_and_apostrophes_in_prose() {
        let mut text = "don't".to_owned();
        assert_eq!(auto_close_edit(&mut text, "dont", 4, Language::Plain), None);
        assert_eq!(text, "don't");
        let mut text = "ab".to_owned();
        assert_eq!(auto_close_edit(&mut text, "", 2, Language::Rust), None);
    }
}

fn search_workspace(root: &Path, query: &str, case_sensitive: bool, whole_word: bool) -> Vec<SearchHit> {
    let needle = if case_sensitive {
        query.to_owned()
    } else {
        query.to_lowercase()
    };
    let mut hits = Vec::new();
    for file in fs_util::walk_files(root, 6000) {
        let Ok(text) = fs_util::read_text_file(&file) else {
            continue;
        };
        if text.len() > 2 * 1024 * 1024 {
            continue;
        }
        for (line_index, line) in text.lines().enumerate() {
            let haystack = if case_sensitive {
                line.to_owned()
            } else {
                line.to_lowercase()
            };
            let mut start = 0;
            while let Some(found) = haystack[start..].find(&needle) {
                let column = start + found;
                if whole_word {
                    let before_ok = column == 0
                        || !haystack[..column]
                            .chars()
                            .next_back()
                            .map(|c| c.is_alphanumeric() || c == '_')
                            .unwrap_or(false);
                    let after = column + needle.len();
                    let after_ok = after >= haystack.len()
                        || !haystack[after..]
                            .chars()
                            .next()
                            .map(|c| c.is_alphanumeric() || c == '_')
                            .unwrap_or(false);
                    if !before_ok || !after_ok {
                        start = column + needle.len().max(1);
                        if start >= haystack.len() {
                            break;
                        }
                        continue;
                    }
                }
                hits.push(SearchHit {
                    path: file.clone(),
                    line: line_index + 1,
                    column: line[..column.min(line.len())].chars().count() + 1,
                    text: line.trim().chars().take(180).collect(),
                });
                if hits.len() >= 240 {
                    return hits;
                }
                start = column + needle.len().max(1);
                if start >= haystack.len() {
                    break;
                }
            }
        }
    }
    hits
}
