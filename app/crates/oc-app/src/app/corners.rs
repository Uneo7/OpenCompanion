//! Rounded window corners drawn by Windows 11 (DWM) for the opaque, undecorated window. No effect elsewhere,
//! and ignored by Windows 10 (square corners there).

pub fn round(cc: &eframe::CreationContext<'_>) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND};
        let Ok(handle) = cc.window_handle() else { return };
        if let RawWindowHandle::Win32(h) = handle.as_raw() {
            let pref = DWMWCP_ROUND;
            // SAFETY: a valid HWND from winit, and a pointer to a u32-sized value of the documented size.
            unsafe {
                DwmSetWindowAttribute(
                    h.hwnd.get() as _,
                    DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                    std::ptr::from_ref(&pref).cast(),
                    std::mem::size_of_val(&pref) as u32,
                );
            }
        }
    }
    #[cfg(not(windows))]
    let _ = cc;
}
