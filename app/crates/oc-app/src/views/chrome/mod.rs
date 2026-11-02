mod window_controls;

use eframe::egui::{self, Align, CursorIcon, Id, Pos2, Rect, Sense, Vec2, ViewportCommand};

use super::{install, prefs, Cx};
use crate::app::{actions::Action, state::Screen};
use crate::i18n::{t_with, tr};
use crate::model::LinkState;
use crate::theme::{self, typography::W};
use crate::widgets::{logo, text, text::st, text_tabs, tooltip};

/// Draws the bar in `bar` (window width × 56). First view drawn in a frame: applies the appearance preference.
pub fn title_bar(ui: &mut egui::Ui, cx: &mut Cx, bar: Rect) {
    let ctx = ui.ctx().clone();
    prefs::apply_theme(&ctx, cx.ui);
    install::backup::watch(&ctx, cx);
    let t = theme::get(&ctx);

    // Drag area first, so every control drawn after it takes precedence.
    let drag = ui.interact(bar, Id::new("titlebar-drag"), Sense::click_and_drag());
    if drag.drag_started_by(egui::PointerButton::Primary) {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }

    // Inset from the window's edges (its corners are rounded): same inset and offset as the window buttons.
    let painter = ui.painter();
    let (inset, cy) = (bar.left() + 26.0, bar.center().y + window_controls::DOWN);
    logo::paint(painter, Rect::from_min_size(Pos2::new(inset, cy - 11.0), Vec2::splat(22.0)));
    let brand = st::inter_(13.5, W::SemiBold).tracking(-0.01);
    text::line(painter, tr("app.opencompanion"), &brand, t.text, inset + 32.0, cy - bar.height() / 2.0, bar.height(), Align::Min);

    let tabs = [tr("app.nav.home"), tr("app.nav.eq"), tr("app.nav.settings")];
    let installing = install::visible(&ctx, cx);
    let locked = installing && install::running(&ctx, cx);
    let current = (!installing).then(|| Screen::ALL.iter().position(|s| *s == cx.ui.screen).unwrap_or(0));
    // The onboarding is the whole app: no tabs until the speaker runs OpenCompanion.
    if !(installing && install::onboarding(&ctx)) {
        if let Some(i) = text_tabs::text_tabs(ui, bar.center() + Vec2::new(0.0, window_controls::DOWN), Id::new("tabs"), &tabs, current, locked) {
            install::leave(&ctx, cx);
            cx.act(Action::Show(Screen::ALL[i]));
        }
    }

    let controls_left = window_controls::show(ui, cx, bar);
    status(ui, cx, Pos2::new(controls_left - 14.0, bar.center().y + window_controls::DOWN));
}

fn status(ui: &mut egui::Ui, cx: &mut Cx, right_center: Pos2) {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let s = cx.device.status();
    let label = match s.stream.bits() {
        Some(bits) => t_with("app.conn.short", &[("bits", &bits)]),
        None => "USB".to_owned(),
    };
    let style = st::mono_(11.5, W::Regular);
    let tw = text::width(&ctx, &label, &style);
    let w = 6.0 + 7.0 + tw;
    let rect = Rect::from_min_size(Pos2::new(right_center.x - w, right_center.y - 8.0), Vec2::new(w, 16.0));
    let reconnecting = s.link == LinkState::Reconnecting;
    let painter = ui.painter();
    painter.circle_filled(Pos2::new(rect.left() + 3.0, rect.center().y), 3.0, if reconnecting { t.warn } else { t.ok });
    text::line(painter, &label, &style, t.muted, rect.left() + 13.0, rect.top(), 16.0, Align::Min);
    let resp = ui.interact(rect, Id::new("conn"), Sense::hover());
    tooltip::attach(&resp, tr(if reconnecting { "app.conn.reconnecting" } else { "app.conn.tip" }), None);
    if resp.hovered() {
        ctx.set_cursor_icon(CursorIcon::Default);
    }
}
