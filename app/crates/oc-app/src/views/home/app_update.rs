//! Home footer for a newer app on GitHub (release builds): "Nouvelle version de l'application X.Y.Z", why when
//! the newest firmware needs it, a "Télécharger" link to the release page (system browser) and a × that hides
//! it until another version appears (remembered in config.json). No self-update: the user installs the download.

use eframe::egui::{self, Id, Rect};

use super::update::{line, Click};
use super::Cx;
use crate::i18n::{t_with, tr};

fn dismissed_id() -> Id {
    Id::new("home-app-update-dismissed")
}

/// Read once from config.json (never in screenshots), then kept in memory.
fn dismissed(ctx: &egui::Context) -> Option<String> {
    if let Some(d) = ctx.data(|m| m.get_temp::<Option<String>>(dismissed_id())) {
        return d;
    }
    let d = if crate::views::prefs::persists(ctx) { crate::config::AppConfig::load().dismissed_app_update } else { None };
    ctx.data_mut(|m| m.insert_temp(dismissed_id(), d.clone()));
    d
}

fn dismiss(ctx: &egui::Context, version: &str) {
    ctx.data_mut(|m| m.insert_temp(dismissed_id(), Some(version.to_owned())));
    if crate::views::prefs::persists(ctx) {
        let _ = crate::config::set_dismissed_app_update(version);
    }
}

pub(super) fn detail(u: &oc_device::install::AppUpdate) -> String {
    u.needed_by_firmware.as_deref().map_or_else(String::new, |tag| t_with("home.app_update.needed", &[("tag", &tag)]))
}

/// `true` when the line is shown (the firmware offer then waits).
pub(super) fn show(ui: &mut egui::Ui, cx: &mut Cx, content: Rect) -> bool {
    let Some(u) = cx.device.app_update() else { return false };
    let ctx = ui.ctx().clone();
    if dismissed(&ctx).as_deref() == Some(u.version.as_str()) {
        return false;
    }
    let title = t_with("home.app_update.available", &[("version", &u.version)]);
    match line(ui, content, &format!("APP={}", u.version), &title, &detail(&u), tr("home.app_update.go")) {
        Some(Click::Go) => {
            if let Err(e) = crate::platform::browser::open_url(&u.url) {
                oc_device::install::log_file::append(&[format!("app update: {e}")]);
                cx.ui.toast.show(&ctx, e);
            }
        }
        Some(Click::Close) => dismiss(&ctx, &u.version),
        None => {}
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use oc_device::install::AppUpdate;

    #[test]
    fn the_line_says_when_the_firmware_needs_the_new_app() {
        let mut u = AppUpdate { version: "1.1.0".into(), url: "https://x".into(), needed_by_firmware: None };
        assert!(detail(&u).is_empty());
        u.needed_by_firmware = Some("v1.1.0".into());
        assert!(detail(&u).contains("v1.1.0"));
        let ctx = egui::Context::default();
        assert_eq!(dismissed(&ctx), None);
        dismiss(&ctx, "1.1.0");
        assert_eq!(dismissed(&ctx).as_deref(), Some("1.1.0"));
    }
}
