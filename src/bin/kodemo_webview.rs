//! Tiny Rust bridge that presents a WebKitGTK view inside Kodemo's X11 window.
//!
//! GTK and WebKit are loaded dynamically so Kodemo builds even on systems
//! without their development headers. The web engine itself is WebKitGTK; all
//! integration and IPC code in Kodemo remains Rust.

use libc::{c_char, c_int, c_ulong, c_void};
use std::ffi::CString;
use std::io::Write;
use std::ptr;

#[cfg(target_os = "linux")]
fn main() {
    if let Err(err) = run() {
        eprintln!("kodemo-webview: {err}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("kodemo-webview is only supported on Linux");
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 7 {
        return Err("usage: kodemo-webview PARENT_XID X Y WIDTH HEIGHT URL".to_owned());
    }
    let parent: c_ulong = args[1]
        .parse()
        .map_err(|_| "invalid parent window id".to_owned())?;
    let x: c_int = args[2].parse().unwrap_or(0);
    let y: c_int = args[3].parse().unwrap_or(0);
    let width: c_int = args[4].parse().unwrap_or(640);
    let height: c_int = args[5].parse().unwrap_or(480);
    let url = CString::new(args[6].clone()).map_err(|_| "URL contains NUL".to_owned())?;

    if std::env::var("KODEMO_SOFTWARE").ok().as_deref() == Some("1") {
        unsafe {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
            std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
        }
    } else {
        // Legacy Intel GPUs are much happier with WebKit's non-composited path.
        // Page content still renders through WebKit's own rasterizer.
        unsafe {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }

    unsafe {
        let gtk_name = CString::new("libgtk-3.so.0").unwrap();
        let webkit_name = CString::new("libwebkit2gtk-4.1.so.0").unwrap();
        let gtk_lib = libc::dlopen(gtk_name.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
        if gtk_lib.is_null() {
            return Err(format!("could not load libgtk-3: {}", dl_error()));
        }
        let webkit_lib =
            libc::dlopen(webkit_name.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
        if webkit_lib.is_null() {
            return Err(format!(
                "could not load libwebkit2gtk-4.1: {}",
                dl_error()
            ));
        }

        let gtk_init_check: unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char) -> c_int =
            load(gtk_lib, "gtk_init_check")?;
        let gtk_window_new: unsafe extern "C" fn(c_int) -> *mut c_void =
            load(gtk_lib, "gtk_window_new")?;
        let gtk_window_set_title: unsafe extern "C" fn(*mut c_void, *const c_char) =
            load(gtk_lib, "gtk_window_set_title")?;
        let gtk_window_set_default_size: unsafe extern "C" fn(*mut c_void, c_int, c_int) =
            load(gtk_lib, "gtk_window_set_default_size")?;
        let gtk_window_set_decorated: unsafe extern "C" fn(*mut c_void, c_int) =
            load(gtk_lib, "gtk_window_set_decorated")?;
        let gtk_window_set_skip_taskbar_hint: unsafe extern "C" fn(*mut c_void, c_int) =
            load(gtk_lib, "gtk_window_set_skip_taskbar_hint")?;
        let gtk_window_set_type_hint: unsafe extern "C" fn(*mut c_void, c_int) =
            load(gtk_lib, "gtk_window_set_type_hint")?;
        let gtk_widget_realize: unsafe extern "C" fn(*mut c_void) =
            load(gtk_lib, "gtk_widget_realize")?;
        let gtk_widget_show_all: unsafe extern "C" fn(*mut c_void) =
            load(gtk_lib, "gtk_widget_show_all")?;
        let gtk_widget_get_window: unsafe extern "C" fn(*mut c_void) -> *mut c_void =
            load(gtk_lib, "gtk_widget_get_window")?;
        let gtk_widget_grab_focus: unsafe extern "C" fn(*mut c_void) =
            load(gtk_lib, "gtk_widget_grab_focus")?;
        let gtk_container_add: unsafe extern "C" fn(*mut c_void, *mut c_void) =
            load(gtk_lib, "gtk_container_add")?;
        let gtk_main: unsafe extern "C" fn() = load(gtk_lib, "gtk_main")?;
        let gdk_x11_window_get_xid: unsafe extern "C" fn(*mut c_void) -> c_ulong =
            load(gtk_lib, "gdk_x11_window_get_xid")?;
        let gdk_window_set_override_redirect: unsafe extern "C" fn(*mut c_void, c_int) =
            load(gtk_lib, "gdk_window_set_override_redirect")?;
        let webkit_web_view_new: unsafe extern "C" fn() -> *mut c_void =
            load(webkit_lib, "webkit_web_view_new")?;
        let webkit_web_view_load_uri: unsafe extern "C" fn(*mut c_void, *const c_char) =
            load(webkit_lib, "webkit_web_view_load_uri")?;

        let argv0 = CString::new(args[0].clone()).unwrap();
        let mut argv_ptrs: Vec<*mut c_char> =
            vec![argv0.as_ptr() as *mut c_char, ptr::null_mut()];
        let mut argc = 1;
        let mut argv_ptr = argv_ptrs.as_mut_ptr();
        if gtk_init_check(&mut argc, &mut argv_ptr) == 0 {
            return Err("GTK couldn't initialize an X11 display".to_owned());
        }

        let title = CString::new("Kodemo Preview").unwrap();
        let window = gtk_window_new(0);
        if window.is_null() {
            return Err("could not create the preview window".to_owned());
        }
        gtk_window_set_title(window, title.as_ptr());
        gtk_window_set_default_size(window, width, height);
        gtk_window_set_decorated(window, 0);
        gtk_window_set_skip_taskbar_hint(window, 1);
        gtk_window_set_type_hint(window, 5); // GDK_WINDOW_TYPE_HINT_UTILITY
        let webview = webkit_web_view_new();
        if webview.is_null() {
            return Err("could not create WebKitWebView".to_owned());
        }
        gtk_container_add(window, webview);
        gtk_widget_realize(window);
        let gdk_window = gtk_widget_get_window(window);
        if gdk_window.is_null() {
            return Err("GTK didn't expose an X11 window".to_owned());
        }
        gdk_window_set_override_redirect(gdk_window, 1);
        let xid = gdk_x11_window_get_xid(gdk_window);
        if xid == 0 {
            return Err("GTK returned an invalid X11 window id".to_owned());
        }

        let xlib = x11_dl::xlib::Xlib::open()
            .map_err(|e| format!("could not load libX11: {e}"))?;
        let display = (xlib.XOpenDisplay)(ptr::null());
        if display.is_null() {
            return Err("could not open the X11 display".to_owned());
        }
        (xlib.XReparentWindow)(display, xid, parent, x, y);
        (xlib.XMoveResizeWindow)(display, xid, x, y, width.max(1) as u32, height.max(1) as u32);
        (xlib.XMapWindow)(display, xid);
        (xlib.XRaiseWindow)(display, xid);
        (xlib.XSync)(display, 0);

        gtk_widget_show_all(window);
        gtk_widget_grab_focus(webview);
        webkit_web_view_load_uri(webview, url.as_ptr());

        println!("KODEMO_WEBVIEW_XID {xid}");
        let _ = std::io::stdout().flush();

        gtk_main();
    }
    Ok(())
}

#[cfg(target_os = "linux")]
unsafe fn load<T: Copy>(library: *mut c_void, name: &str) -> Result<T, String> {
    let c_name = CString::new(name).map_err(|_| "invalid symbol name".to_owned())?;
    let symbol = libc::dlsym(library, c_name.as_ptr());
    if symbol.is_null() {
        return Err(format!("missing symbol {name}: {}", dl_error()));
    }
    if std::mem::size_of::<T>() != std::mem::size_of::<*mut c_void>() {
        return Err(format!("unexpected function-pointer size for {name}"));
    }
    Ok(std::mem::transmute_copy::<*mut c_void, T>(&symbol))
}

#[cfg(target_os = "linux")]
unsafe fn dl_error() -> String {
    let error = libc::dlerror();
    if error.is_null() {
        "unknown dynamic linker error".to_owned()
    } else {
        std::ffi::CStr::from_ptr(error)
            .to_string_lossy()
            .into_owned()
    }
}
