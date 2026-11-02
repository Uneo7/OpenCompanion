use eframe::egui::{self, CursorIcon, Id, Pos2, Rect, Response, Sense, Vec2};

use crate::theme::{self, motion, Theme};

pub const SIZE: Vec2 = Vec2::new(40.0, 24.0);

pub fn switch(ui: &mut egui::Ui, min: Pos2, id: Id, on: &mut bool) -> Response {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let rect = Rect::from_min_size(min, SIZE);
    let mut resp = ui.interact(rect, id, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    if resp.hovered() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
    paint_switch(ui.painter(), &ctx, rect, id, *on, resp.hovered(), &t);
    resp
}

pub fn switch_off(ui: &mut egui::Ui, min: Pos2, id: Id, on: bool) -> Response {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let rect = Rect::from_min_size(min, SIZE);
    let resp = ui.interact(rect, id, Sense::hover());
    let painter = crate::widgets::paint::faded(ui.painter(), t.disabled);
    paint_switch(&painter, &ctx, rect, id, on, false, &t);
    resp
}

pub fn paint_switch(painter: &egui::Painter, ctx: &egui::Context, rect: Rect, id: Id, on: bool, hovered: bool, t: &Theme) {
    paint_toggle(painter, ctx, rect, id, on, hovered, t, 3.0);
}

/// Shared drawing of `.switch` and `.tg`: `inset` is the gap between the track and the thumb.
#[allow(clippy::too_many_arguments)]
pub fn paint_toggle(painter: &egui::Painter, ctx: &egui::Context, rect: Rect, id: Id, on: bool, hovered: bool, t: &Theme, inset: f32) {
    let k = motion::toggle(ctx, id.with("switch"), on, 0.2, motion::SPRING);
    let c = motion::toggle(ctx, id.with("switch-c"), on, 0.2, motion::SPRING);
    let h = motion::toggle(ctx, id.with("switch-h"), hovered, 0.2, motion::SPRING);
    let r = rect.height() / 2.0;
    painter.rect_filled(rect, r, theme::mix(t.track, t.fill, c));
    let thumb = r - inset;
    let travel = rect.width() - 2.0 * r;
    let center = Pos2::new(rect.left() + r + travel * k, rect.center().y);
    let rad = thumb * (1.0 + 0.06 * h);
    painter.circle_filled(center + Vec2::new(0.0, 1.0), rad + 0.8, theme::alpha(t.thumb_shadow, 0.6));
    painter.circle_filled(center, rad, t.thumb);
}
