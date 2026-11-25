//! A small built-in file browser used for Open File / Open Folder / Save As.
//! It avoids a native dialog dependency while still feeling like part of the IDE.

use crate::fs_util::{self, Entry};
use crate::i18n::t;
use crate::icons::{self, Icon};
use crate::settings::{home_dir, Lang};
use crate::theme::{alpha, ThemePalette};
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Frame, Id, Key, Layout, Margin, Modal, Rect, RichText,
    ScrollArea, Sense, Stroke, TextEdit, Ui, Vec2,
};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerMode {
    OpenFile,
    OpenFolder,
    SaveFile,
}

impl PickerMode {
    fn title(self, lang: Lang) -> &'static str {
        match self {
            Self::OpenFile => t(lang, "Open File", "Abrir arquivo"),
            Self::OpenFolder => t(lang, "Open Folder", "Abrir pasta"),
            Self::SaveFile => t(lang, "Save File As", "Salvar arquivo como"),
        }
    }

    fn confirm(self, lang: Lang) -> &'static str {
        match self {
            Self::OpenFile => t(lang, "Open", "Abrir"),
            Self::OpenFolder => t(lang, "Open Folder", "Abrir pasta"),
            Self::SaveFile => t(lang, "Save", "Salvar"),
        }
    }
}

pub struct FilePicker {
    pub open: bool,
    pub mode: PickerMode,
    pub cwd: PathBuf,
    entries: Vec<Entry>,
    selected: Option<PathBuf>,
    pub file_name: String,
    path_input: String,
    error: Option<String>,
    show_hidden: bool,
    workspace: Option<PathBuf>,
    just_opened: bool,
    refresh_needed: bool,
    pub hovered: Option<(PathBuf, Rect)>,
}

impl Default for FilePicker {
    fn default() -> Self {
        Self {
            open: false,
            mode: PickerMode::OpenFile,
            cwd: home_dir(),
            entries: Vec::new(),
            selected: None,
            file_name: String::new(),
            path_input: String::new(),
            error: None,
            show_hidden: false,
            workspace: None,
            just_opened: false,
            refresh_needed: true,
            hovered: None,
        }
    }
}

impl FilePicker {
    pub fn open(&mut self, mode: PickerMode, start: Option<PathBuf>, workspace: Option<PathBuf>) {
        self.open = true;
        self.mode = mode;
        self.workspace = workspace;
        let mut dir = start
            .filter(|p| p.is_dir())
            .or_else(|| self.workspace.clone())
            .unwrap_or_else(home_dir);
        if !dir.is_dir() {
            dir = home_dir();
        }
        self.cwd = dir;
        self.selected = None;
        self.error = None;
        self.show_hidden = false;
        self.just_opened = true;
        self.refresh_needed = true;
        if mode == PickerMode::SaveFile && self.file_name.trim().is_empty() {
            self.file_name = "untitled.txt".to_owned();
        }
    }

    fn refresh(&mut self) {
        match fs_util::list_dir(&self.cwd, self.show_hidden) {
            Ok(entries) => {
                self.entries = entries;
                self.error = None;
            }
            Err(err) => {
                self.entries.clear();
                self.error = Some(err);
            }
        }
        self.path_input = self.cwd.display().to_string();
        self.refresh_needed = false;
    }

    fn navigate(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.cwd = path;
            self.selected = None;
            self.refresh_needed = true;
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        palette: &ThemePalette,
        lang: Lang,
    ) -> Option<PathBuf> {
        if !self.open {
            return None;
        }
        self.hovered = None;
        if self.refresh_needed {
            self.refresh();
        }
        let mut result = None;
        let mut close = false;
        let frame = Frame::NONE
            .fill(palette.panel)
            .stroke(Stroke::new(1.0_f32, palette.border_strong))
            .corner_radius(10.0)
            .inner_margin(Margin::symmetric(14, 12));
        let modal = Modal::new(Id::new("kodemo-file-picker"))
            .frame(frame)
            .backdrop_color(Color32::from_black_alpha(120))
            .show(ctx, |ui| {
                ui.set_width(660.0);
                ui.set_height(480.0);
                ui.horizontal(|ui| {
                    icons::draw(
                        ui.painter(),
                        Rect::from_center_size(ui.cursor().min + Vec2::new(9.0, 10.0), Vec2::splat(16.0)),
                        if self.mode == PickerMode::OpenFolder {
                            Icon::FolderOpen
                        } else {
                            Icon::File
                        },
                        palette.accent,
                        1.3,
                    );
                    ui.add_space(24.0);
                    ui.label(
                        RichText::new(self.mode.title(lang))
                            .size(15.0)
                            .strong()
                            .color(palette.fg),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icon_button(ui, Icon::Close, &palette, t(lang, "Close", "Fechar")).clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(10.0);

                // Path bar.
                ui.horizontal(|ui| {
                    let up = ui
                        .add(
                            egui::Button::new(RichText::new("↑").color(palette.fg))
                                .fill(palette.panel_alt)
                                .corner_radius(4.0),
                        )
                        .on_hover_text(t(lang, "Up one folder", "Subir uma pasta"));
                    if up.clicked() {
                        if let Some(parent) = self.cwd.parent() {
                            self.navigate(parent.to_path_buf());
                        }
                    }
                    if icon_button(ui, Icon::Refresh, &palette, t(lang, "Refresh", "Atualizar")).clicked() {
                        self.refresh_needed = true;
                    }
                    let response = ui.add(
                        TextEdit::singleline(&mut self.path_input)
                            .font(FontId::monospace(12.0))
                            .desired_width(f32::INFINITY)
                            .margin(Margin::symmetric(6, 4)),
                    );
                    if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        self.goto_typed_path();
                    }
                });

                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for (label, path) in [
                        ("~", home_dir()),
                        (t(lang, "Documents", "Documentos"), home_dir().join("Documentos")),
                        (t(lang, "Downloads", "Downloads"), home_dir().join("Downloads")),
                        (
                            t(lang, "Workspace", "Workspace"),
                            self.workspace.clone().unwrap_or_else(home_dir),
                        ),
                        ("/", PathBuf::from("/")),
                    ] {
                        if path.is_dir() && ui.small_button(label).clicked() {
                            self.navigate(path);
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let hidden = ui
                            .selectable_label(
                                self.show_hidden,
                                t(lang, "Hidden files", "Arquivos ocultos"),
                            );
                        if hidden.clicked() {
                            self.show_hidden = !self.show_hidden;
                            self.refresh_needed = true;
                        }
                    });
                });
                ui.separator();

                let list_height = if self.mode == PickerMode::SaveFile {
                    330.0
                } else {
                    350.0
                };
                ScrollArea::vertical()
                    .id_salt("kodemo-file-picker-list")
                    .max_height(list_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if let Some(err) = &self.error {
                            ui.label(RichText::new(err).color(palette.error).size(12.0));
                        }
                        let entries = self.entries.clone();
                        for entry in &entries {
                            self.render_entry(ui, entry, palette, &mut result, &mut close);
                        }
                    });

                if let Some(err) = &self.error {
                    ui.label(RichText::new(err).color(palette.error).size(11.5));
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if self.mode == PickerMode::SaveFile {
                        ui.label(
                            RichText::new(t(lang, "File name:", "Nome do arquivo:"))
                                .color(palette.fg_muted)
                                .size(12.0),
                        );
                        ui.add(
                            TextEdit::singleline(&mut self.file_name)
                                .desired_width(220.0)
                                .font(FontId::monospace(12.0))
                                .margin(Margin::symmetric(6, 4)),
                        );
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(self.mode.confirm(lang)).color(palette.accent_fg),
                                )
                                .fill(palette.accent)
                                .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            self.confirm(&mut result, lang);
                        }
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(t(lang, "Cancel", "Cancelar")).color(palette.fg),
                                )
                                .fill(palette.panel_alt)
                                .corner_radius(4.0),
                            )
                            .clicked()
                        {
                            close = true;
                        }
                    });
                });
            });

        if modal.backdrop_response.clicked() {
            close = true;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            close = true;
        }
        if close {
            self.open = false;
        }
        if let Some(path) = &result {
            self.open = false;
            return Some(path.clone());
        }
        None
    }

    fn goto_typed_path(&mut self) {
        let path = PathBuf::from(self.path_input.trim());
        if path.is_dir() {
            self.cwd = path;
            self.selected = None;
            self.refresh_needed = true;
        } else if path.is_file() {
            if let Some(parent) = path.parent() {
                self.cwd = parent.to_path_buf();
                self.selected = Some(path);
                self.refresh_needed = true;
            }
        } else {
            self.error = Some(format!(
                "{} does not exist.",
                path.display()
            ));
        }
    }

    fn render_entry(
        &mut self,
        ui: &mut Ui,
        entry: &Entry,
        palette: &ThemePalette,
        result: &mut Option<PathBuf>,
        close: &mut bool,
    ) {
        if self.mode == PickerMode::OpenFolder && !entry.is_dir {
            return;
        }
        if self.mode == PickerMode::OpenFile && entry.is_dir {
            // Directories are still shown so the user can navigate into them.
        }
        let selected = self.selected.as_deref() == Some(entry.path.as_path());
        let row_height = 25.0;
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), row_height),
            Sense::click(),
        );
        if selected {
            ui.painter().rect_filled(rect, 3.0, alpha(palette.accent, 40));
        } else if response.hovered() {
            ui.painter().rect_filled(rect, 3.0, palette.hover);
        }
        if response.hovered()
            && !entry.is_dir
            && crate::preview::kind_for(&entry.path).is_some()
        {
            self.hovered = Some((entry.path.clone(), rect));
        }
        let icon = if entry.is_dir {
            Icon::Folder
        } else {
            icons::for_path(&entry.path)
        };
        let color = if entry.is_dir {
            palette.syntax_tag
        } else {
            crate::explorer::file_color(&entry.path, palette)
        };
        icons::draw(
            ui.painter(),
            Rect::from_center_size(
                egui::pos2(rect.left() + 16.0, rect.center().y),
                Vec2::splat(14.0),
            ),
            icon,
            color,
            1.2,
        );
        let prefix = if entry.is_dir { "› " } else { "  " };
        ui.painter().text(
            egui::pos2(rect.left() + 30.0, rect.center().y),
            Align2::LEFT_CENTER,
            format!("{prefix}{}", entry.name),
            FontId::proportional(13.0),
            if selected { palette.fg } else { palette.fg_muted },
        );
        if let Some(modified) = entry.modified {
            if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                let date = duration.as_secs();
                let text = format_date(date);
                ui.painter().text(
                    egui::pos2(rect.right() - 10.0, rect.center().y),
                    Align2::RIGHT_CENTER,
                    text,
                    FontId::proportional(10.5),
                    palette.fg_faint,
                );
            }
        }
        if response.double_clicked() {
            if entry.is_dir {
                self.navigate(entry.path.clone());
            } else if self.mode == PickerMode::SaveFile {
                self.file_name = entry.name.clone();
                self.selected = Some(entry.path.clone());
            } else if self.mode == PickerMode::OpenFile {
                *result = Some(entry.path.clone());
                *close = true;
            }
        } else if response.clicked() {
            self.selected = Some(entry.path.clone());
        }
    }

    fn confirm(&mut self, result: &mut Option<PathBuf>, _lang: Lang) {
        match self.mode {
            PickerMode::OpenFile => {
                if let Some(path) = self
                    .selected
                    .clone()
                    .filter(|p| p.is_file())
                    .or_else(|| {
                        let name = self.path_input.trim();
                        let path = PathBuf::from(name);
                        path.is_file().then_some(path)
                    })
                {
                    *result = Some(path);
                } else {
                    self.error = Some("Select a file to open.".to_owned());
                }
            }
            PickerMode::OpenFolder => {
                let path = self
                    .selected
                    .clone()
                    .filter(|p| p.is_dir())
                    .unwrap_or_else(|| self.cwd.clone());
                *result = Some(path);
            }
            PickerMode::SaveFile => {
                let name = self.file_name.trim();
                if name.is_empty() {
                    self.error = Some("Type a file name.".to_owned());
                    return;
                }
                if name.contains('/') || name.contains('\\') {
                    self.error = Some("The file name cannot contain slashes.".to_owned());
                    return;
                }
                *result = Some(self.cwd.join(name));
            }
        }
    }

    #[allow(dead_code)]
    pub fn selected_path(&self) -> Option<&Path> {
        self.selected.as_deref()
    }
}

fn icon_button(
    ui: &mut Ui,
    icon: Icon,
    palette: &ThemePalette,
    tooltip: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 4.0, palette.hover);
    }
    icons::draw(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(13.0)),
        icon,
        palette.fg_muted,
        1.15,
    );
    response.on_hover_text(tooltip)
}

fn format_date(unix_seconds: u64) -> String {
    // A tiny civil-date conversion; enough for a file list and dependency-free.
    let days = (unix_seconds / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}
