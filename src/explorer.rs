use crate::fs_util::{self, Entry};
use crate::icons::{self, Icon};
use crate::settings::Lang;
use crate::theme::{alpha, ThemePalette};
use eframe::egui::{self, Color32, FontId, Rect, Sense, Ui, Vec2};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub enum ExplorerAction {
    OpenFile(PathBuf),
    NewFile(PathBuf),
    NewFolder(PathBuf),
    Rename(PathBuf),
    Delete(PathBuf),
    CopyPath(PathBuf),
    Reveal(PathBuf),
    ToggleHidden,
    Refresh,
    OpenFolder,
}

pub struct Explorer {
    pub root: Option<PathBuf>,
    pub expanded: HashSet<PathBuf>,
    pub selected: Option<PathBuf>,
    pub show_hidden: bool,
    pub hovered: Option<(PathBuf, Rect)>,
    children: HashMap<PathBuf, Vec<Entry>>,
    errors: HashMap<PathBuf, String>,
}

impl Default for Explorer {
    fn default() -> Self {
        Self {
            root: None,
            expanded: HashSet::new(),
            selected: None,
            show_hidden: false,
            hovered: None,
            children: HashMap::new(),
            errors: HashMap::new(),
        }
    }
}

impl Explorer {
    pub fn set_root(&mut self, root: Option<PathBuf>) {
        self.root = root;
        self.expanded.clear();
        self.selected = None;
        self.children.clear();
        self.errors.clear();
        let hidden = self.show_hidden;
        if let Some(root) = self.root.clone() {
            self.ensure_loaded(&root, hidden);
        }
    }

    pub fn set_show_hidden(&mut self, show_hidden: bool) {
        if self.show_hidden != show_hidden {
            self.show_hidden = show_hidden;
            self.refresh();
        }
    }

    pub fn refresh(&mut self) {
        self.children.clear();
        self.errors.clear();
        let hidden = self.show_hidden;
        if let Some(root) = self.root.clone() {
            self.ensure_loaded(&root, hidden);
            let expanded: Vec<PathBuf> = self.expanded.iter().cloned().collect();
            for path in expanded {
                self.ensure_loaded(&path, hidden);
            }
        }
    }

    pub fn refresh_dir(&mut self, dir: &Path) {
        self.children.remove(dir);
        self.errors.remove(dir);
        let hidden = self.show_hidden;
        if dir.exists() {
            self.ensure_loaded(dir, hidden);
        }
    }

    fn ensure_loaded(&mut self, path: &Path, show_hidden: bool) {
        if self.children.contains_key(path) {
            return;
        }
        match fs_util::list_dir(path, show_hidden) {
            Ok(entries) => {
                self.children.insert(path.to_path_buf(), entries);
                self.errors.remove(path);
            }
            Err(err) => {
                self.errors.insert(path.to_path_buf(), err);
            }
        }
    }

    pub fn selected_parent(&self) -> PathBuf {
        if let Some(selected) = &self.selected {
            if selected.is_dir() {
                return selected.clone();
            }
            if let Some(parent) = selected.parent() {
                return parent.to_path_buf();
            }
        }
        self.root.clone().unwrap_or_else(|| crate::settings::home_dir())
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        palette: &ThemePalette,
        lang: Lang,
        show_hidden: bool,
        active_path: Option<&Path>,
        gutter: f32,
    ) -> Option<ExplorerAction> {
        if self.show_hidden != show_hidden {
            self.set_show_hidden(show_hidden);
        }
        self.hovered = None;
        let Some(root) = self.root.clone() else {
            ui.add_space(16.0);
            let mut open = false;
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(if lang.is_pt() {
                        "Nenhuma pasta aberta"
                    } else {
                        "No folder opened"
                    })
                    .color(palette.fg_muted)
                    .size(12.5),
                );
                ui.add_space(8.0);
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(if lang.is_pt() {
                                "Abrir pasta"
                            } else {
                                "Open Folder"
                            })
                            .color(palette.accent_fg),
                        )
                        .fill(palette.accent)
                        .corner_radius(3.0),
                    )
                    .clicked()
                {
                    open = true;
                }
            });
            return open.then_some(ExplorerAction::OpenFolder);
        };

        let root_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace");
        ui.add_space(2.0);
        let mut action = None;
        let hidden = self.show_hidden;
        self.ensure_loaded(&root, hidden);
        let entries = self.children.get(&root).cloned().unwrap_or_default();
        if let Some(err) = self.errors.get(&root) {
            ui.horizontal(|ui| {
                ui.add_space(gutter);
                ui.label(egui::RichText::new(err).color(palette.error).size(11.5));
            });
        } else if entries.is_empty() {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(gutter);
                ui.label(
                    egui::RichText::new(if lang.is_pt() {
                        "A pasta está vazia."
                    } else {
                        "The folder is empty."
                    })
                    .color(palette.fg_faint)
                    .size(12.0)
                    .italics(),
                );
            });
        }
        let indent = 12.0;
        for entry in &entries {
            self.render_entry(
                ui,
                entry,
                root_name,
                0,
                indent,
                gutter,
                palette,
                lang,
                show_hidden,
                active_path,
                &mut action,
            );
            if action.is_some() {
                break;
            }
        }
        action
    }

    #[allow(clippy::too_many_arguments)]
    fn render_entry(
        &mut self,
        ui: &mut Ui,
        entry: &Entry,
        root_name: &str,
        depth: usize,
        indent: f32,
        gutter: f32,
        palette: &ThemePalette,
        lang: Lang,
        show_hidden: bool,
        active_path: Option<&Path>,
        action: &mut Option<ExplorerAction>,
    ) {
        if entry.hidden && !show_hidden {
            return;
        }
        let width = ui.available_width().max(80.0);
        let height = 21.0;
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
        let is_active = active_path == Some(entry.path.as_path());
        let is_selected = self.selected.as_deref() == Some(entry.path.as_path());
        let hovered = response.hovered();
        if is_active || is_selected {
            ui.painter().rect_filled(
                rect,
                3.0,
                alpha(palette.accent, if is_active { 52 } else { 32 }),
            );
            if is_active {
                ui.painter().rect_filled(
                    Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())),
                    0.0,
                    palette.accent,
                );
            }
        } else if hovered {
            ui.painter().rect_filled(rect, 3.0, palette.hover);
        }

        let base_x = rect.left() + gutter.max(4.0) + depth as f32 * indent;
        let icon_rect = Rect::from_center_size(
            egui::pos2(base_x + 8.0, rect.center().y),
            Vec2::splat(14.0),
        );
        if entry.is_dir {
            let open = self.expanded.contains(&entry.path);
            icons::draw(
                ui.painter(),
                icon_rect,
                if open { Icon::FolderOpen } else { Icon::Folder },
                palette.fg_muted,
                1.2,
            );
            let chevron = Rect::from_center_size(
                egui::pos2(base_x - 2.0, rect.center().y),
                Vec2::splat(10.0),
            );
            icons::draw(
                ui.painter(),
                chevron,
                if open {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                },
                palette.fg_muted,
                1.1,
            );
        } else {
            let color = file_color(&entry.path, palette);
            icons::draw(
                ui.painter(),
                icon_rect,
                icons::for_path(&entry.path),
                color,
                1.1,
            );
        }

        let text_x = base_x + 19.0;
        let text_color = if is_active {
            palette.fg
        } else if entry.is_dir {
            palette.fg
        } else {
            palette.fg
        };
        let max_w = (rect.right() - text_x - 6.0).max(10.0);
        let clip = Rect::from_min_max(
            egui::pos2(text_x, rect.top()),
            egui::pos2(text_x + max_w, rect.bottom()),
        );
        let painter = ui.painter_at(clip);
        let font = FontId::proportional(12.8);
        painter.text(
            egui::pos2(text_x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            &entry.name,
            font,
            text_color,
        );

        if hovered && !entry.is_dir && crate::preview::kind_for(&entry.path).is_some() {
            self.hovered = Some((entry.path.clone(), rect));
        } else {
            response.clone().on_hover_text(entry.path.display().to_string());
        }

        if response.clicked() {
            self.selected = Some(entry.path.clone());
            if entry.is_dir {
                if self.expanded.contains(&entry.path) {
                    self.expanded.remove(&entry.path);
                } else {
                    self.expanded.insert(entry.path.clone());
                    let hidden = self.show_hidden;
                    self.ensure_loaded(&entry.path, hidden);
                }
            } else {
                *action = Some(ExplorerAction::OpenFile(entry.path.clone()));
            }
        }

        let path = entry.path.clone();
        response.context_menu(|ui| {
            ui.set_min_width(190.0);
            if ui
                .button(if lang.is_pt() { "Novo arquivo…" } else { "New File…" })
                .clicked()
            {
                *action = Some(ExplorerAction::NewFile(if path.is_dir() {
                    path.clone()
                } else {
                    path.parent().unwrap_or(&path).to_path_buf()
                }));
                ui.close_menu();
            }
            if ui
                .button(if lang.is_pt() {
                    "Nova pasta…"
                } else {
                    "New Folder…"
                })
                .clicked()
            {
                *action = Some(ExplorerAction::NewFolder(if path.is_dir() {
                    path.clone()
                } else {
                    path.parent().unwrap_or(&path).to_path_buf()
                }));
                ui.close_menu();
            }
            ui.separator();
            if ui
                .button(if lang.is_pt() { "Renomear…" } else { "Rename…" })
                .clicked()
            {
                *action = Some(ExplorerAction::Rename(path.clone()));
                ui.close_menu();
            }
            if ui
                .button(if lang.is_pt() { "Excluir" } else { "Delete" })
                .clicked()
            {
                *action = Some(ExplorerAction::Delete(path.clone()));
                ui.close_menu();
            }
            if ui
                .button(if lang.is_pt() {
                    "Copiar caminho"
                } else {
                    "Copy Path"
                })
                .clicked()
            {
                *action = Some(ExplorerAction::CopyPath(path.clone()));
                ui.close_menu();
            }
            if ui
                .button(if lang.is_pt() {
                    "Mostrar no gerenciador"
                } else {
                    "Reveal in File Manager"
                })
                .clicked()
            {
                *action = Some(ExplorerAction::Reveal(path.clone()));
                ui.close_menu();
            }
            ui.separator();
            if ui
                .button(if show_hidden {
                    if lang.is_pt() {
                        "Ocultar arquivos ocultos"
                    } else {
                        "Hide Hidden Files"
                    }
                } else if lang.is_pt() {
                    "Mostrar arquivos ocultos"
                } else {
                    "Show Hidden Files"
                })
                .clicked()
            {
                *action = Some(ExplorerAction::ToggleHidden);
                ui.close_menu();
            }
        });

        if entry.is_dir && self.expanded.contains(&entry.path) {
            let entries = self.children.get(&entry.path).cloned().unwrap_or_default();
            for child in entries {
                self.render_entry(
                    ui,
                    &child,
                    root_name,
                    depth + 1,
                    indent,
                    gutter,
                    palette,
                    lang,
                    show_hidden,
                    active_path,
                    action,
                );
                if action.is_some() {
                    break;
                }
            }
        }

        let _ = root_name;
    }
}

pub fn file_color(path: &Path, palette: &ThemePalette) -> Color32 {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" => Color32::from_rgb(222, 165, 132),
        "py" => Color32::from_rgb(255, 212, 59),
        "js" | "mjs" | "cjs" => Color32::from_rgb(240, 219, 79),
        "ts" | "tsx" => Color32::from_rgb(81, 162, 255),
        "jsx" => Color32::from_rgb(97, 218, 251),
        "html" | "htm" => Color32::from_rgb(227, 111, 62),
        "css" | "scss" | "sass" | "less" => Color32::from_rgb(86, 156, 214),
        "json" | "jsonc" | "json5" => Color32::from_rgb(200, 200, 80),
        "xml" | "xsl" | "xslt" | "xsd" | "plist" => Color32::from_rgb(150, 190, 240),
        "cs" => Color32::from_rgb(170, 130, 220),
        "lock" => Color32::from_rgb(150, 155, 165),
        "md" | "markdown" => Color32::from_rgb(81, 154, 234),
        "php" => Color32::from_rgb(137, 147, 190),
        "c" | "h" | "cpp" | "cc" | "hpp" => Color32::from_rgb(100, 160, 220),
        "go" => Color32::from_rgb(90, 200, 220),
        "java" => Color32::from_rgb(220, 120, 100),
        "toml" | "yaml" | "yml" => palette.syntax_tag,
        "svg" => Color32::from_rgb(255, 184, 108),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "avif" | "tif"
        | "tiff" => Color32::from_rgb(180, 140, 220),
        "wav" | "mp3" | "flac" | "ogg" | "m4a" | "aac" | "opus" => {
            Color32::from_rgb(239, 149, 190)
        }
        "mp4" | "mov" | "webm" | "mkv" | "avi" => Color32::from_rgb(228, 128, 146),
        "pdf" => Color32::from_rgb(232, 112, 112),
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" => {
            Color32::from_rgb(214, 178, 116)
        }
        "doc" | "docx" | "odt" | "rtf" => Color32::from_rgb(116, 166, 230),
        "xls" | "xlsx" | "csv" | "ods" => Color32::from_rgb(116, 200, 146),
        "ppt" | "pptx" | "odp" => Color32::from_rgb(232, 146, 116),
        "sh" | "bash" | "zsh" | "fish" | "bat" | "cmd" | "ps1" => {
            Color32::from_rgb(137, 220, 137)
        }
        _ => palette.fg_muted,
    }
}
