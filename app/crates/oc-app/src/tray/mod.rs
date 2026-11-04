//! The tray (Windows notification area, macOS menu bar, Linux StatusNotifierItem). Closing the window hides it
//! there and the app keeps running; the icon shows the window again and its menu quits. Without a tray
//! (screenshots, a Linux desktop without StatusNotifierItem host), closing quits as before.
//!
//! The tray events arrive outside egui's frame (the OS event loop, or ksni's thread on Linux): they only set
//! flags here, and [`watch`] applies them on the next frame. A hidden window may get no frame on macOS, so the
//! window is also brought back natively there ([`native::show`]).

#[cfg(any(windows, target_os = "macos"))]
#[path = "desktop.rs"]
mod backend;
#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod backend;
mod icon;
mod native;

pub use native::event_loop_hook;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use eframe::egui::{Context, ViewportCommand};

static READY: AtomicBool = AtomicBool::new(false);
static SHOW: AtomicBool = AtomicBool::new(false);
static QUIT: AtomicBool = AtomicBool::new(false);
static CTX: OnceLock<Context> = OnceLock::new();

/// Keeps the icon alive.
pub struct Tray {
    _backend: Option<backend::Backend>,
}

pub fn start(cc: &eframe::CreationContext<'_>) -> Tray {
    let _ = CTX.set(cc.egui_ctx.clone());
    native::remember(cc);
    native::activate();
    let backend = match backend::spawn(request_show, request_quit) {
        Ok(b) => {
            READY.store(true, Ordering::SeqCst);
            Some(b)
        }
        Err(e) => {
            oc_device::install::log_file::append(&[format!("tray unavailable ({e}): closing the window quits")]);
            None
        }
    };
    Tray { _backend: backend }
}

/// A tray icon exists: closing the window hides it instead of quitting.
pub fn available() -> bool {
    READY.load(Ordering::SeqCst)
}

/// Hides the window (Wayland cannot hide a window: it is minimised there).
pub fn hide(ctx: &Context) {
    ctx.send_viewport_cmd(ViewportCommand::Visible(false));
    if cfg!(target_os = "linux") {
        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
    }
}

/// Every frame: shows the window when the tray asked, and says whether it asked to quit.
pub fn watch(ctx: &Context) -> bool {
    if SHOW.swap(false, Ordering::SeqCst) {
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
    }
    QUIT.swap(false, Ordering::SeqCst)
}

/// Also from a second launch (any thread: the native part only acts on the main thread).
pub fn request_show() {
    SHOW.store(true, Ordering::SeqCst);
    native::show();
    if let Some(ctx) = CTX.get() {
        ctx.request_repaint();
    }
}

/// Quitting goes through the window (it refuses while a firmware is being written): shown first.
fn request_quit() {
    QUIT.store(true, Ordering::SeqCst);
    request_show();
}
