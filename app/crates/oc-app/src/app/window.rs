//! The native window: one fixed size (no resize, no maximise, no fullscreen), the app's own title bar and the
//! "Node" icon (window and taskbar).
//!
//! The system decorations are off on all three platforms; the title bar drags the window
//! (`ViewportCommand::StartDrag`) and draws its own minimise and close buttons, which winit supports on
//! Windows, macOS, X11 and Wayland. The window is transparent so its corners can be rounded, except on Linux
//! where compositors may not honour transparency (see `theme::metrics::window_radius`).

use eframe::egui::ViewportBuilder;

use crate::theme::metrics::{WINDOW_H, WINDOW_W};

pub fn viewport() -> ViewportBuilder {
    let builder = match crate::assets::app_icon::icon_data() {
        Some(icon) => ViewportBuilder::default().with_icon(icon),
        None => ViewportBuilder::default(),
    };
    builder
        .with_title("OpenCompanion")
        .with_app_id("opencompanion")
        .with_inner_size([WINDOW_W, WINDOW_H])
        .with_min_inner_size([WINDOW_W, WINDOW_H])
        .with_max_inner_size([WINDOW_W, WINDOW_H])
        .with_resizable(false)
        .with_maximize_button(false)
        .with_fullscreen(false)
        .with_decorations(false)
        .with_transparent(cfg!(target_os = "macos"))
}

/// eframe options for the app window. Screenshots (`--shots`) open it on the primary monitor only: at its
/// origin (the primary display is at (0, 0) of the desktop on Windows, macOS and X11), not focused, not in the
/// taskbar, never on another screen.
pub fn native_options(shots: bool) -> eframe::NativeOptions {
    if shots {
        let viewport = viewport().with_position([0.0, 0.0]).with_active(false).with_taskbar(false);
        return eframe::NativeOptions { viewport, centered: false, renderer: RENDERER, wgpu_options: wgpu_options(), ..Default::default() };
    }
    eframe::NativeOptions { viewport: viewport(), centered: true, renderer: RENDERER, wgpu_options: wgpu_options(), ..Default::default() }
}

/// The native API of each platform only (on Windows DX12 alone: NVIDIA ships its Vulkan driver in the same DLL as
/// its OpenGL one, nvoglv64.dll). wgpu's default also enumerates OpenGL, which loads that driver anyway.
fn backends() -> eframe::wgpu::Backends {
    use eframe::wgpu::Backends;
    if cfg!(target_os = "windows") {
        Backends::DX12
    } else if cfg!(target_os = "macos") {
        Backends::METAL
    } else {
        Backends::VULKAN
    }
}

fn wgpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
    use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
    let mut create = WgpuSetupCreateNew::default();
    create.instance_descriptor.backends = backends();
    WgpuConfiguration { wgpu_setup: WgpuSetup::CreateNew(create), ..Default::default() }
}

/// wgpu (DirectX 12 / Metal / Vulkan) first: a broken OpenGL driver must not take the app down. OpenGL stays
/// compiled in as the fallback (`main.rs` retries with it if wgpu cannot start).
pub const RENDERER: eframe::Renderer = eframe::Renderer::Wgpu;
