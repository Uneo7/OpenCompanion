//! Home footer when an update is available: a thin rule, then an icon, "Mise à jour disponible" and the versions
//! that change ("DSP 0.2.5 → 0.2.6 · USB 0.5.4 → 0.5.5"), and at the right a "Mettre à jour" link (the installer,
//! update flow) and a small × that hides it until another version appears (remembered in config.json). A newer
//! app comes first ([`super::app_update`]).

use eframe::egui::{self, Align, CursorIcon, Id, Pos2, Rect, Sense, Vec2};
use oc_device::install::{Chip, Flow, UpdateCheck};

use super::Cx;
use crate::assets::icons;
use crate::i18n::tr;
use crate::model::format::display_version;
use crate::theme::{self, motion, typography::W};
use crate::views::install;
use crate::widgets::{paint, text, text::st};

pub const HEIGHT: f32 = 44.0;

/// The chips that change, as "DSP 0.2.5 → 0.2.6", in a stable order.
fn changes(check: &UpdateCheck) -> Vec<(&'static str, String, String)> {
    [(Chip::Dsp, "DSP"), (Chip::Usb, "USB")]
        .into_iter()
        .filter_map(|(chip, name)| {
            let u = check.chip(chip).filter(|u| u.available)?;
            let from = u.from.as_deref().map_or("?", display_version).to_owned();
            let to = u.to.as_deref().map_or("?", display_version).to_owned();
            Some((name, from, to))
        })
        .collect()
}

/// Identity of an offer (its target versions): a dismissed offer comes back only with another version.
pub fn offer_key(check: &UpdateCheck) -> Option<String> {
    let c = changes(check);
    (!c.is_empty()).then(|| c.iter().map(|(n, _, to)| format!("{n}={to}")).collect::<Vec<_>>().join(" "))
}

pub fn summary(check: &UpdateCheck) -> String {
    changes(check).iter().map(|(n, from, to)| format!("{n} {from} → {to}")).collect::<Vec<_>>().join(" · ")
}

fn dismissed_id() -> Id {
    Id::new("home-update-dismissed")
}

/// The dismissed offer: read once from config.json (never in screenshots), then kept in memory.
pub fn dismissed(ctx: &egui::Context) -> Option<String> {
    if let Some(d) = ctx.data(|m| m.get_temp::<Option<String>>(dismissed_id())) {
        return d;
    }
    let d = if crate::views::prefs::persists(ctx) { crate::config::AppConfig::load().dismissed_update } else { None };
    ctx.data_mut(|m| m.insert_temp(dismissed_id(), d.clone()));
    d
}

pub fn dismiss(ctx: &egui::Context, key: &str) {
    ctx.data_mut(|m| m.insert_temp(dismissed_id(), Some(key.to_owned())));
    if crate::views::prefs::persists(ctx) {
        let _ = crate::config::set_dismissed_update(key);
    }
}

/// What the user did on a footer line.
pub(super) enum Click {
    Go,
    Close,
}

/// A footer line at the bottom of `content`: icon, `title`, `detail` (mono), an accent link `go` and a ×.
/// `fade` identifies the offer (a new one fades in).
pub(super) fn line(ui: &mut egui::Ui, content: Rect, fade: &str, title: &str, detail: &str, go: &str) -> Option<Click> {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let hash = fade.bytes().fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(u64::from(b)));
    let k = motion::EASE.at(motion::since(&ctx, Id::new("home-update-fade"), hash, 0.35));
    let painter = paint::faded(ui.painter(), k);
    let r = Rect::from_min_max(Pos2::new(content.left(), content.bottom() - HEIGHT), content.max);
    painter.hline(r.x_range(), r.top(), egui::Stroke::new(1.0_f32, t.line));
    let cy = r.top() + HEIGHT / 2.0 + 2.0;

    icons::UPDATE.paint(&painter, Pos2::new(r.left() + 8.0, cy), 16.0, t.a1);
    let label = text::line(&painter, title, &st::inter_(13.0, W::Medium), t.muted, r.left() + 24.0, cy - 9.0, 18.0, Align::Min);
    text::line(&painter, detail, &st::mono_(12.0, W::Regular), t.muted, label.right() + 10.0, cy - 9.0, 18.0, Align::Min);

    // ×, then the link to its left.
    let close = Rect::from_center_size(Pos2::new(r.right() - 10.0, cy), Vec2::splat(20.0));
    let cresp = ui.interact(close, Id::new("home-update-close"), Sense::click());
    let cc = if cresp.hovered() { t.text } else { t.muted };
    icons::CLOSE.paint(&painter, close.center(), 14.0, cc);
    crate::widgets::tooltip::attach(&cresp, tr("home.update.dismiss"), None);
    let lstyle = st::inter_(13.0, W::SemiBold);
    let lw = text::width(&ctx, go, &lstyle);
    let link = Rect::from_min_size(Pos2::new(close.left() - 14.0 - lw, cy - 10.0), Vec2::new(lw, 20.0));
    let lresp = ui.interact(link, Id::new("home-update-go"), Sense::click());
    text::line(&painter, go, &lstyle, t.a1, link.left(), cy - 9.0, 18.0, Align::Min);
    if lresp.hovered() {
        painter.hline(link.x_range(), cy + 9.0, egui::Stroke::new(1.0_f32, t.a1));
    }
    if lresp.hovered() || cresp.hovered() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
    if lresp.clicked() {
        Some(Click::Go)
    } else if cresp.clicked() {
        Some(Click::Close)
    } else {
        None
    }
}

/// The footer, at the bottom of `content`: a newer app first (a newer firmware may need it), else the firmware
/// offer, when not dismissed.
pub fn show(ui: &mut egui::Ui, cx: &mut Cx, content: Rect) {
    if super::app_update::show(ui, cx, content) {
        return;
    }
    let check = cx.device.updates();
    let Some(key) = offer_key(&check) else { return };
    let ctx = ui.ctx().clone();
    if dismissed(&ctx).as_deref() == Some(key.as_str()) {
        return;
    }
    match line(ui, content, &key, tr("home.update.available"), &summary(&check), tr("home.update.go")) {
        Some(Click::Go) => install::open(&ctx, cx, Flow::Update),
        Some(Click::Close) => dismiss(&ctx, &key),
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oc_device::install::ChipUpdate;

    fn offer(dsp: Option<(&str, &str)>, usb: Option<(&str, &str)>) -> UpdateCheck {
        let u = |chip, v: Option<(&str, &str)>| ChipUpdate {
            chip,
            from: v.map(|(f, _)| f.to_owned()),
            from_stock: false,
            from_probable: false,
            to: v.map(|(_, t)| t.to_owned()),
            available: v.is_some(),
        };
        UpdateCheck::Done(vec![u(Chip::Dsp, dsp), u(Chip::Usb, usb)])
    }

    #[test]
    fn the_offer_lists_the_chips_that_change() {
        let both = offer(Some(("0.2.5", "0.2.6")), Some(("0.5.4", "0.5.5")));
        assert_eq!(summary(&both), "DSP 0.2.5 → 0.2.6 · USB 0.5.4 → 0.5.5");
        assert_eq!(offer_key(&both).as_deref(), Some("DSP=0.2.6 USB=0.5.5"));
        let dsp = offer(Some(("0.2.5", "0.2.6")), None);
        assert_eq!(summary(&dsp), "DSP 0.2.5 → 0.2.6");
        assert_ne!(offer_key(&dsp), offer_key(&offer(Some(("0.2.5", "0.2.7")), None)), "a newer version is a new offer");
        assert_eq!(offer_key(&offer(None, None)), None);
    }

    #[test]
    fn a_dismissed_offer_stays_hidden_until_another_version() {
        let ctx = egui::Context::default();
        assert_eq!(dismissed(&ctx), None);
        dismiss(&ctx, "DSP=0.2.6");
        assert_eq!(dismissed(&ctx).as_deref(), Some("DSP=0.2.6"));
        assert_ne!(dismissed(&ctx), offer_key(&offer(Some(("0.2.5", "0.2.7")), None)));
    }
}
