use eframe::egui::Context;

use super::Cx;
use crate::app::actions::Action;
use crate::app::files::Pick;
use crate::i18n::{t_with, tr};
use crate::widgets::dialog::{Body, Dialog};

#[derive(Debug, Clone, PartialEq)]
pub enum ViewAction {
    AskExport(usize),
    /// "Import into this profile…": a profile file for profile `i`, then the replace confirmation.
    AskImport(usize),
    Import(usize, Box<oc_device::params::Profile>),
    RecoveryGuide,
    Dismiss,
}

fn open(cx: &mut Cx, title: String, body: String, ok: &str, danger: bool, action: ViewAction) {
    let serial = cx.ui.next_serial();
    cx.ui.menu = None;
    cx.ui.dialog = Some(Dialog { title, body: Body::Text(body), ok: ok.into(), danger, action: Action::View(action), serial });
}

/// Applies `a`; `body` is the dialog's content when it comes from an OK button. Returns false to keep that
/// dialog open.
pub fn apply(ctx: &Context, cx: &mut Cx, a: ViewAction, _body: Option<&Body>) -> bool {
    match a {
        ViewAction::AskExport(i) => {
            cx.ui.menu = None;
            cx.ui.pick = Some(Pick::ExportProfile(i));
        }
        ViewAction::AskImport(i) => {
            cx.ui.menu = None;
            cx.ui.pick = Some(Pick::ImportProfile(i));
        }
        ViewAction::Import(i, p) => {
            let name = cx.profile_name(i);
            if cx.device.import_profile(i, &p) {
                cx.ui.eq.dirty = false;
                let scaled = cx.ui.eq.load(cx.device.profiles()[i].bands.clone());
                let done = t_with("profile.import.done", &[("name", &name)]);
                cx.ui.toast.show(ctx, if scaled { format!("{done} · {}", tr("eq.boost.scaled")) } else { done });
            } else {
                cx.ui.toast.show(ctx, t_with("profile.import.error", &[("error", &tr("profile.import.refused"))]));
            }
        }
        ViewAction::Dismiss => {}
        ViewAction::RecoveryGuide => {
            open(cx, tr("inst.err.guide").into(), tr("inst.guide.body").into(), tr("app.close.ok"), false, ViewAction::Dismiss);
        }
    }
    true
}
