use std::sync::{Arc, Mutex};

use eframe::egui::{Context, Id};

use super::install::Session;

pub use crate::config::ThemePref;

#[derive(Default)]
pub struct ViewState {
    /// `None` until [`super::prefs::init`] (then Automatique).
    pub theme: Option<ThemePref>,
    /// Appearance changes are saved in the app config (not in screenshot mode).
    pub persist_theme: bool,
    pub applied: Option<(ThemePref, bool)>,
    pub install: Option<Session>,
    /// The user left the installer view: a failed hardware flow does not reopen it by itself.
    pub install_dismissed: bool,
    pub backup_tried: bool,
}

type Shared = Arc<Mutex<ViewState>>;

fn key() -> Id {
    Id::NULL.with("oc-view-state")
}

fn shared(ctx: &Context) -> Shared {
    ctx.data_mut(|d| d.get_temp_mut_or_insert_with(key(), || Arc::new(Mutex::new(ViewState::default()))).clone())
}

pub fn with<R>(ctx: &Context, f: impl FnOnce(&mut ViewState) -> R) -> R {
    let s = shared(ctx);
    let mut g = s.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut g)
}

pub fn reset(ctx: &Context) {
    with(ctx, |s| *s = ViewState::default());
}
