pub const WINDOW_W: f32 = 1100.0;
pub const WINDOW_H: f32 = 720.0;
pub const TOOLBAR_H: f32 = 56.0;
pub const CONTENT_W: f32 = 920.0;

pub const S6: f32 = 32.0;

/// Corner radius the app paints. Only macOS gets a transparent window (Metal): on Windows the window is opaque
/// (DX12 swapchains are opaque) and Windows 11 rounds it itself (`app::corners`); Linux stays square.
pub const fn window_radius() -> f32 {
    if cfg!(target_os = "macos") {
        12.0
    } else {
        0.0
    }
}
