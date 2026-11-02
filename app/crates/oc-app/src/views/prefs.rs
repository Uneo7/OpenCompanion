use eframe::egui::Context;

use super::state::{self, ThemePref};
use crate::app::state::UiState;
use crate::i18n::{self, Lang};
use crate::platform::theme::os_light;

/// Sets the start-up appearance: the saved preference (or one forced on the command line). `persist`: later
/// changes are saved.
pub fn init(ctx: &Context, pref: ThemePref, persist: bool) {
    state::with(ctx, |s| {
        s.theme = Some(pref);
        s.persist_theme = persist;
        s.applied = None;
    });
}

pub fn apply_theme(ctx: &Context, ui: &mut UiState) {
    let os_light = os_light(ctx) == Some(true);
    state::with(ctx, |s| {
        let pref = *s.theme.get_or_insert(ThemePref::Auto);
        if s.applied != Some((pref, os_light)) {
            s.applied = Some((pref, os_light));
            ui.light = match pref {
                ThemePref::Auto => os_light,
                ThemePref::Dark => false,
                ThemePref::Light => true,
            };
        }
    });
}

pub fn theme(ctx: &Context) -> ThemePref {
    state::with(ctx, |s| s.theme.unwrap_or_default())
}

/// Sets the appearance (applied on the next frame) and saves it (a read-only config dir only loses the
/// persistence).
pub fn set_theme(ctx: &Context, pref: ThemePref) {
    let persist = state::with(ctx, |s| {
        s.theme = Some(pref);
        s.persist_theme
    });
    if persist {
        let _ = crate::config::set_theme(pref);
    }
}

/// Choices are saved to config.json (never in screenshots).
pub fn persists(ctx: &Context) -> bool {
    state::with(ctx, |s| s.persist_theme)
}

pub fn set_lang(lang: Lang) {
    if crate::config::set_language(lang).is_err() {
        i18n::set_lang(lang);
    }
}
