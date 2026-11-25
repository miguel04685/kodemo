//! Floating previews for image and audio files in the explorer and file picker.
//!
//! Images are decoded on a worker thread and cached as a GPU texture, so
//! hovering a large photo never blocks the UI. Audio starts after a short
//! dwell time with `ffplay` (or `paplay` for WAV) and stops when the pointer
//! leaves the file.

use crate::i18n::t;
use crate::icons::{self, Icon};
use crate::settings::Lang;
use crate::theme::{self, ThemePalette};
use eframe::egui::{
    self, Color32, Id, Margin, Order, Pos2, Rect, RichText, Stroke, Vec2,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant, SystemTime};

const IMAGE_DELAY: Duration = Duration::from_millis(220);
const AUDIO_DELAY: Duration = Duration::from_millis(520);
const IMAGE_BOX: Vec2 = Vec2::new(300.0, 230.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Image,
    Audio,
}

pub fn kind_for(path: &Path) -> Option<Kind> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" => Some(Kind::Image),
        "mp3" | "wav" | "ogg" | "oga" | "flac" | "m4a" | "aac" | "opus" => Some(Kind::Audio),
        _ => None,
    }
}

struct CachedImage {
    path: PathBuf,
    modified: Option<SystemTime>,
    texture: egui::TextureHandle,
    size: Vec2,
}

struct PendingImage {
    path: PathBuf,
    rx: Receiver<Result<DecodedImage, String>>,
}

struct DecodedImage {
    pixels: Vec<u8>,
    width: usize,
    height: usize,
    modified: Option<SystemTime>,
}

struct AudioPreview {
    path: PathBuf,
    child: Child,
}

pub struct HoverPreview {
    target: Option<PathBuf>,
    since: Option<Instant>,
    image: Option<CachedImage>,
    pending: Option<PendingImage>,
    image_error: Option<PathBuf>,
    audio: Option<AudioPreview>,
    audio_failed: Option<PathBuf>,
}

impl Default for HoverPreview {
    fn default() -> Self {
        Self {
            target: None,
            since: None,
            image: None,
            pending: None,
            image_error: None,
            audio: None,
            audio_failed: None,
        }
    }
}

impl Drop for HoverPreview {
    fn drop(&mut self) {
        self.stop_audio();
    }
}

impl HoverPreview {
    /// `target` is the hovered file and the screen rectangle of its row.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        target: Option<(PathBuf, Rect)>,
        enabled: bool,
        audio_enabled: bool,
        lang: Lang,
        palette: &ThemePalette,
    ) {
        let target = if enabled {
            target.filter(|(path, _)| kind_for(path).is_some())
        } else {
            None
        };
        let target_path = target.as_ref().map(|(path, _)| path.clone());
        if target_path != self.target {
            self.stop_audio();
            self.since = None;
            self.audio_failed = None;
            self.target = target_path;
        }
        let Some((path, row)) = target else {
            self.stop_audio();
            return;
        };
        let Some(kind) = kind_for(&path) else {
            return;
        };
        let since = *self.since.get_or_insert_with(Instant::now);
        let waited = since.elapsed();
        let needed = match kind {
            Kind::Image => IMAGE_DELAY,
            Kind::Audio => Duration::from_millis(120),
        };
        if waited < needed {
            ctx.request_repaint_after(needed - waited + Duration::from_millis(16));
            return;
        }

        match kind {
            Kind::Image => {
                self.poll_image(ctx);
                self.ensure_image(ctx, &path);
                if let Some(pending) = &self.pending {
                    if pending.path == path {
                        ctx.request_repaint_after(Duration::from_millis(75));
                    }
                }
                self.show_image(ctx, &path, row, lang, palette);
            }
            Kind::Audio => {
                let playing =
                    self.audio.as_ref().map(|a| a.path.as_path()) == Some(path.as_path());
                if audio_enabled && !playing && self.audio_failed.as_deref() != Some(path.as_path())
                {
                    if waited >= AUDIO_DELAY {
                        self.ensure_audio(&path);
                    } else {
                        ctx.request_repaint_after(
                            AUDIO_DELAY - waited + Duration::from_millis(16),
                        );
                    }
                }
                self.show_audio(ctx, &path, row, lang, palette, audio_enabled);
            }
        }
        self.reap_audio();
    }

    fn poll_image(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.pending else {
            return;
        };
        let Ok(result) = pending.rx.try_recv() else {
            return;
        };
        let path = pending.path.clone();
        self.pending = None;
        match result {
            Ok(image) => {
                let color = egui::ColorImage::from_rgba_unmultiplied(
                    [image.width, image.height],
                    &image.pixels,
                );
                let texture = ctx.load_texture(
                    format!("kodemo-hover:{}", path.display()),
                    color,
                    egui::TextureOptions::LINEAR,
                );
                self.image = Some(CachedImage {
                    path,
                    modified: image.modified,
                    size: Vec2::new(image.width as f32, image.height as f32),
                    texture,
                });
                self.image_error = None;
            }
            Err(_) => self.image_error = Some(path),
        }
    }

    fn ensure_image(&mut self, ctx: &egui::Context, path: &Path) {
        let modified = fs::metadata(path).ok().and_then(|m| m.modified().ok());
        if let Some(cache) = &self.image {
            if cache.path == path && cache.modified == modified {
                return;
            }
        }
        if self.image_error.as_deref() == Some(path) {
            return;
        }
        if let Some(pending) = &self.pending {
            if pending.path == path {
                return;
            }
        }
        let (tx, rx) = channel();
        let load_path = path.to_path_buf();
        let thread_path = load_path.clone();
        std::thread::Builder::new()
            .name("kodemo-image-preview".to_owned())
            .spawn(move || {
                let modified = fs::metadata(&thread_path)
                    .ok()
                    .and_then(|m| m.modified().ok());
                let result = fs::read(&thread_path)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| {
                        image::load_from_memory(&bytes)
                            .map_err(|e| e.to_string())
                            .map(|img| img.thumbnail(IMAGE_BOX.x as u32, IMAGE_BOX.y as u32))
                    })
                    .map(|img| {
                        let rgba = img.to_rgba8();
                        let (width, height) = rgba.dimensions();
                        DecodedImage {
                            pixels: rgba.into_raw(),
                            width: width as usize,
                            height: height as usize,
                            modified,
                        }
                    });
                let _ = tx.send(result);
            })
            .ok();
        self.pending = Some(PendingImage { path: load_path, rx });
        ctx.request_repaint_after(Duration::from_millis(40));
    }

    fn show_image(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
        row: Rect,
        lang: Lang,
        palette: &ThemePalette,
    ) {
        let cached = self
            .image
            .as_ref()
            .filter(|image| image.path == path)
            .map(|image| (image.texture.id(), image.size));
        let loading = self.pending.as_ref().map(|p| p.path == path).unwrap_or(false);
        let failed = self.image_error.as_deref() == Some(path);
        let (width, height) = match cached {
            Some((_, size)) => {
                let scale = (size.x / IMAGE_BOX.x).max(size.y / IMAGE_BOX.y).max(1.0);
                let fitted = size / scale;
                (fitted.x + 18.0, fitted.y + 42.0)
            }
            None => (266.0, loading.then_some(92.0).unwrap_or(76.0)),
        };
        let pos = float_pos(ctx, row, width, height);
        egui::Area::new(Id::new("kodemo-hover-image"))
            .order(Order::Tooltip)
            .interactable(false)
            .fixed_pos(pos)
            .constrain(true)
            .show(ctx, |ui| {
                popup_frame(palette).show(ui, |ui| {
                    ui.set_width(width - 18.0);
                    let name = file_name(path);
                    if let Some((texture, size)) = cached {
                        let scale = (size.x / IMAGE_BOX.x).max(size.y / IMAGE_BOX.y).max(1.0);
                        let fitted = size / scale;
                        ui.add(
                            egui::Image::new((texture, fitted))
                                .corner_radius(5)
                                .max_size(fitted),
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(name)
                                    .size(11.5)
                                    .strong()
                                    .color(palette.fg),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "{} × {}",
                                            size.x as u32, size.y as u32
                                        ))
                                        .size(9.6)
                                        .color(palette.fg_faint),
                                    );
                                },
                            );
                        });
                    } else if failed {
                        ui.horizontal(|ui| {
                            icons::draw(
                                ui.painter(),
                                Rect::from_min_size(ui.cursor().min, Vec2::splat(14.0)),
                                Icon::Warning,
                                palette.warning,
                                1.2,
                            );
                            ui.add_space(18.0);
                            ui.label(
                                RichText::new(name)
                                    .size(11.5)
                                    .strong()
                                    .color(palette.fg),
                            );
                        });
                        ui.label(
                            RichText::new(t(
                                lang,
                                "This image could not be previewed.",
                                "Não foi possível pré-visualizar esta imagem.",
                            ))
                            .size(9.8)
                            .color(palette.fg_muted),
                        );
                    } else {
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new().size(14.0));
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new(name)
                                    .size(11.5)
                                    .strong()
                                    .color(palette.fg),
                            );
                        });
                        ui.label(
                            RichText::new(t(lang, "Loading preview…", "Carregando prévia…"))
                                .size(9.8)
                                .color(palette.fg_muted),
                        );
                    }
                });
            });
    }

    fn ensure_audio(&mut self, path: &Path) {
        if self.audio.as_ref().map(|a| a.path.as_path()) == Some(path) {
            return;
        }
        self.stop_audio();
        let mut child = Command::new("ffplay")
            .args(["-nodisp", "-autoexit", "-loglevel", "quiet", "-volume", "70"])
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok();
        if child.is_none() {
            child = Command::new("paplay")
                .arg(path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .ok();
        }
        if let Some(child) = child {
            self.audio = Some(AudioPreview {
                path: path.to_path_buf(),
                child,
            });
        } else {
            self.audio_failed = Some(path.to_path_buf());
        }
    }

    fn reap_audio(&mut self) {
        let Some(audio) = &mut self.audio else {
            return;
        };
        if !matches!(audio.child.try_wait(), Ok(None)) {
            self.audio = None;
        }
    }

    fn stop_audio(&mut self) {
        if let Some(mut audio) = self.audio.take() {
            let _ = audio.child.kill();
            let _ = audio.child.wait();
        }
    }

    fn show_audio(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
        row: Rect,
        lang: Lang,
        palette: &ThemePalette,
        audio_enabled: bool,
    ) {
        let playing = self.audio.as_ref().map(|a| a.path.as_path()) == Some(path);
        let name = file_name(path);
        let size = fs::metadata(path)
            .map(|m| format_size(m.len()))
            .unwrap_or_default();
        let width = 272.0;
        let height = 104.0;
        let pos = float_pos(ctx, row, width, height);
        egui::Area::new(Id::new("kodemo-hover-audio"))
            .order(Order::Tooltip)
            .interactable(false)
            .fixed_pos(pos)
            .constrain(true)
            .show(ctx, |ui| {
                popup_frame(palette).show(ui, |ui| {
                    ui.set_width(width - 18.0);
                    ui.horizontal(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::splat(38.0), egui::Sense::hover());
                        ui.painter().rect_filled(
                            rect,
                            8.0,
                            theme::alpha(palette.accent, if playing { 46 } else { 26 }),
                        );
                        icons::draw(
                            ui.painter(),
                            Rect::from_center_size(rect.center(), Vec2::splat(17.0)),
                            if playing { Icon::Stop } else { Icon::Play },
                            palette.accent,
                            1.5,
                        );
                        ui.add_space(2.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(name)
                                    .size(12.0)
                                    .strong()
                                    .color(palette.fg),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{} · {}",
                                    t(lang, "Audio", "Áudio"),
                                    size
                                ))
                                .size(9.8)
                                .color(palette.fg_faint),
                            );
                        });
                    });
                    ui.add_space(6.0);
                    draw_waveform(ui, path, palette, playing, width - 18.0);
                    ui.add_space(4.0);
                    let status = if !audio_enabled {
                        t(
                            lang,
                            "Hover audio is disabled in Settings.",
                            "O áudio ao passar o mouse está desativado nas Configurações.",
                        )
                    } else if playing {
                        t(lang, "Playing preview…", "Reproduzindo prévia…")
                    } else {
                        t(lang, "Hover to hear a preview", "Passe o mouse para ouvir uma prévia")
                    };
                    ui.label(
                        RichText::new(status)
                            .size(9.4)
                            .color(if playing { palette.accent } else { palette.fg_muted }),
                    );
                });
            });
    }
}

fn float_pos(ctx: &egui::Context, row: Rect, width: f32, height: f32) -> Pos2 {
    let screen = ctx.screen_rect();
    let gap = 10.0;
    let mut x = row.right() + gap;
    if x + width > screen.right() - 8.0 {
        x = row.left() - width - gap;
    }
    x = x.clamp(screen.left() + 8.0, (screen.right() - width - 8.0).max(screen.left() + 8.0));
    let y = row.top()
        .clamp(screen.top() + 8.0, (screen.bottom() - height - 8.0).max(screen.top() + 8.0));
    Pos2::new(x, y)
}

fn popup_frame(palette: &ThemePalette) -> egui::Frame {
    egui::Frame::NONE
        .fill(palette.panel)
        .stroke(Stroke::new(1.0_f32, palette.border_strong))
        .corner_radius(8)
        .inner_margin(Margin::same(9))
        .shadow(egui::epaint::Shadow {
            offset: [0, 7],
            blur: 20,
            spread: 0,
            color: Color32::from_black_alpha(88),
        })
}

fn draw_waveform(ui: &mut egui::Ui, path: &Path, palette: &ThemePalette, playing: bool, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 20.0), egui::Sense::hover());
    let bars = 34usize;
    let gap = 2.0;
    let bar_width = ((rect.width() - gap * (bars - 1) as f32) / bars as f32).max(1.0);
    let mut seed = crate::util::fnv1a(path.to_string_lossy().as_bytes());
    let color = if playing {
        palette.accent
    } else {
        theme::alpha(palette.accent, 110)
    };
    for index in 0..bars {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let wave = ((seed % 1000) as f32 / 1000.0).sqrt();
        let h = 5.0 + wave * (rect.height() - 4.0);
        let x = rect.left() + index as f32 * (bar_width + gap);
        let bar = Rect::from_center_size(
            Pos2::new(x + bar_width * 0.5, rect.center().y),
            Vec2::new(bar_width, h),
        );
        ui.painter().rect_filled(bar, bar_width * 0.5, color);
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file")
        .to_owned()
}

fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let value = bytes as f64;
    if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}
