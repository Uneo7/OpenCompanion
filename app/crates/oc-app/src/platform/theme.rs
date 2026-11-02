//! - Windows and macOS: winit reports the system theme and its changes (eframe's [`Context::system_theme`]).
//! - Linux: winit reports nothing (X11 has no such setting, Wayland only the decoration theme), so the app reads
//!   `org.freedesktop.appearance color-scheme` from the XDG settings portal (D-Bus, the same portal client as the
//!   file dialogs) on a background thread, and watches it for changes. No portal, or no preference: dark.

use eframe::egui::{Context, Theme};

/// `Some(true)` when the OS asks for a light appearance, `None` when it says nothing.
pub fn os_light(ctx: &Context) -> Option<bool> {
    #[cfg(target_os = "linux")]
    if let Some(light) = portal::light(ctx) {
        return Some(light);
    }
    ctx.system_theme().map(|t| t == Theme::Light)
}

#[cfg(target_os = "linux")]
mod portal {
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::Once;

    use ashpd::desktop::settings::{ColorScheme, Settings};
    use eframe::egui::Context;
    use futures_util::StreamExt;

    const UNKNOWN: u8 = 0;
    const DARK: u8 = 1;
    const LIGHT: u8 = 2;

    static SCHEME: AtomicU8 = AtomicU8::new(UNKNOWN);
    static WATCH: Once = Once::new();

    fn store(s: ColorScheme) {
        let v = match s {
            ColorScheme::PreferDark => DARK,
            ColorScheme::PreferLight => LIGHT,
            ColorScheme::NoPreference => UNKNOWN,
        };
        SCHEME.store(v, Ordering::Relaxed);
    }

    async fn watch(ctx: &Context) -> ashpd::Result<()> {
        let settings = Settings::new().await?;
        store(settings.color_scheme().await?);
        ctx.request_repaint();
        let mut changes = settings.receive_color_scheme_changed().await?;
        while let Some(s) = changes.next().await {
            store(s);
            ctx.request_repaint();
        }
        Ok(())
    }

    pub fn light(ctx: &Context) -> Option<bool> {
        WATCH.call_once(|| {
            let ctx = ctx.clone();
            // No portal (minimal window managers): the thread ends and the setting stays unknown.
            let _ = std::thread::Builder::new().name("oc-os-theme".into()).spawn(move || {
                let _ = pollster::block_on(watch(&ctx));
            });
        });
        match SCHEME.load(Ordering::Relaxed) {
            DARK => Some(false),
            LIGHT => Some(true),
            _ => None,
        }
    }
}
