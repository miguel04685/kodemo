//! Kodemo — a small native IDE written in Rust with egui.
//!
//! The application tries the hardware OpenGL path first. If the driver cannot
//! create a context, it relaunches itself with Mesa's software rasterizer so
//! old machines (like the Intel HD 2000 this was built on) still get a usable
//! IDE instead of a crash dialog.

mod ai;
mod app;
mod assist;
mod explorer;
mod file_picker;
mod fs_util;
mod i18n;
mod icons;
mod native_picker;
mod preview;
mod preview_server;
mod run;
mod settings;
mod spell;
mod syntax;
mod system;
mod templates;
mod terminal;
mod theme;
mod util;
mod webview;
mod welcome;

use app::KodemoApp;
use settings::Settings;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut settings = Settings::load();
    settings.normalize();
    if let Some(path) = args
        .iter()
        .skip(1)
        .map(PathBuf::from)
        .find(|path| path.is_dir())
    {
        settings.last_workspace = Some(path.to_string_lossy().to_string());
    }
    let forced_software = args.iter().any(|arg| arg == "--software");
    let child = std::env::var_os("KODEMO_SOFTWARE_CHILD").is_some();
    let software = forced_software
        || child
        || settings.software_rendering
        || std::env::var_os("LIBGL_ALWAYS_SOFTWARE").is_some();
    if software {
        std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
    }

    // Pick a window that fits even when the window manager declines to
    // maximize the window (Xfce on small screens is known to do that).
    let (screen_width, screen_height) = screen_size().unwrap_or((1366, 768));
    let initial_width = (screen_width as f32 - 80.0).clamp(760.0, 1400.0);
    let initial_height = (screen_height as f32 - 140.0).clamp(480.0, 900.0);
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Kodemo")
        .with_app_id("kodemo")
        .with_inner_size([initial_width, initial_height])
        .with_min_inner_size([760.0, 480.0]);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        vsync: settings.vsync,
        multisampling: 0,
        // Always let glutin pick the best available config. In software mode
        // `LIBGL_ALWAYS_SOFTWARE=1` makes Mesa expose the llvmpipe renderer;
        // requesting `HardwareAcceleration::Off` here would instead require a
        // config flagged as non-accelerated, which Mesa often does not expose.
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,
        run_and_return: true,
        ..Default::default()
    };

    let app_settings = settings.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        eframe::run_native(
            "Kodemo",
            options,
            Box::new(move |cc| Ok(Box::new(KodemoApp::new(cc, app_settings, software)))),
        )
    }));

    match result {
        Ok(Ok(())) => {}
        Ok(Err(reason)) => {
            eprintln!("Kodemo: hardware rendering failed: {reason}");
            if !software && relaunch_software(&args) {
                eprintln!("Kodemo: relaunching with software OpenGL…");
                return;
            }
            eprintln!(
                "Kodemo: could not start. Try running with `--software` or installing a Mesa \
                 software renderer (libgl1-mesa-dri)."
            );
            std::process::exit(1);
        }
        Err(_) => {
            // A panic in application code is not a rendering failure. Restarting
            // in software mode would only hide it and could lose unsaved work.
            eprintln!("Kodemo: unexpected panic; exiting.");
            std::process::exit(101);
        }
    }
}

fn relaunch_software(args: &[String]) -> bool {
    if std::env::var_os("KODEMO_SOFTWARE_CHILD").is_some() {
        return false;
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    Command::new(exe)
        .args(args.iter().skip(1))
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        .env("KODEMO_SOFTWARE_CHILD", "1")
        .spawn()
        .is_ok()
}

/// Screen size in pixels, used to avoid opening a window taller than the
/// display on small laptops.
fn screen_size() -> Option<(i32, i32)> {
    let xlib = x11_dl::xlib::Xlib::open().ok()?;
    unsafe {
        let display = (xlib.XOpenDisplay)(std::ptr::null());
        if display.is_null() {
            return None;
        }
        let screen = (xlib.XDefaultScreen)(display);
        let width = (xlib.XDisplayWidth)(display, screen);
        let height = (xlib.XDisplayHeight)(display, screen);
        (xlib.XCloseDisplay)(display);
        (width > 0 && height > 0).then_some((width, height))
    }
}

/// Rasterized from `assets/icon.svg`; X11 window icons are pixel buffers.
fn window_icon() -> Option<eframe::egui::IconData> {
    let image = image::load_from_memory(include_bytes!("../assets/kodemo-icon.png")).ok()?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(eframe::egui::IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    })
}
