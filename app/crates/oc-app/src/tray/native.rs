//! Shows the hidden window through the OS, from the tray event (a hidden window may get no egui frame to apply
//! a viewport command). Windows and macOS; Linux relies on the viewport commands of `tray::watch`.

#[cfg(any(windows, target_os = "macos"))]
use std::sync::atomic::{AtomicIsize, Ordering};

/// The native window (HWND on Windows, NSView on macOS), 0 until known.
#[cfg(any(windows, target_os = "macos"))]
static HANDLE: AtomicIsize = AtomicIsize::new(0);

pub fn remember(cc: &eframe::CreationContext<'_>) {
    #[cfg(any(windows, target_os = "macos"))]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Ok(handle) = cc.window_handle() else { return };
        let raw = match handle.as_raw() {
            #[cfg(windows)]
            RawWindowHandle::Win32(h) => h.hwnd.get(),
            #[cfg(target_os = "macos")]
            RawWindowHandle::AppKit(h) => h.ns_view.as_ptr() as isize,
            _ => 0,
        };
        HANDLE.store(raw, Ordering::SeqCst);
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    let _ = cc;
}

/// Called on the main thread (the tray's event handler runs there on Windows and macOS).
pub fn show() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW};
        let hwnd = HANDLE.load(Ordering::SeqCst);
        if hwnd != 0 {
            // SAFETY: the HWND of the app's window, alive as long as the process.
            unsafe {
                ShowWindow(hwnd as _, SW_SHOW);
                ShowWindow(hwnd as _, SW_RESTORE);
                SetForegroundWindow(hwnd as _);
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSApplication, NSView};
        let Some(mtm) = MainThreadMarker::new() else { return };
        let view = HANDLE.load(Ordering::SeqCst) as *const NSView;
        // SAFETY: the NSView of the app's window (winit keeps it alive), used on the main thread.
        if let Some(window) = unsafe { view.as_ref() }.and_then(NSView::window) {
            window.makeKeyAndOrderFront(None);
        }
        #[allow(deprecated)]
        NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
    }
}

/// macOS: an accessory app is not brought to the front at launch; its window is.
pub fn activate() {
    #[cfg(target_os = "macos")]
    if let Some(mtm) = objc2::MainThreadMarker::new() {
        #[allow(deprecated)]
        objc2_app_kit::NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
    }
}

/// macOS: the app lives in the menu bar only, never in the Dock (an "accessory" app).
pub fn event_loop_hook() -> Option<eframe::EventLoopBuilderHook> {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        Some(Box::new(|b| {
            b.with_activation_policy(ActivationPolicy::Accessory);
        }))
    }
    #[cfg(not(target_os = "macos"))]
    None
}
