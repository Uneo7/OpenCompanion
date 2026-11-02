use eframe::egui::{self, Align, Color32, Id, Pos2, Rect, Shape, Stroke, Vec2};

use super::profiles::caption;
use crate::app::actions::Action;
use crate::assets::icons;
use crate::i18n::tr;
use crate::theme::{self, motion, typography::W};
use crate::views::Cx;
use crate::widgets::{button::IconButton, fmt, slider, text, text::st};

/// Caption (16) + number row (4 + 60 + 6) + slider (32).
pub const HEIGHT: f32 = 16.0 + 70.0 + 32.0;

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, column: Rect, top: f32) {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let group = Rect::from_min_size(Pos2::new(column.left(), top), Vec2::new(column.width(), HEIGHT));
    let muted = cx.device.muted();
    let mut vol = cx.device.volume();
    let m = motion::toggle(&ctx, Id::new("vol-muted"), muted, 0.25, motion::EASE);

    caption(ui, tr("home.volume"), group.left(), top);

    let num_top = top + 16.0 + 4.0;
    let num_style = st::inter_(52.0, W::SemiBold).tracking(-0.035);
    let unit_style = st::mono_(12.0, W::Regular);
    let num_g = text::galley(&ctx, &vol.to_string(), &num_style, theme::alpha(t.text, 1.0 - 0.45 * m));
    let num_rect = text::place(ui.painter(), num_g, group.left(), num_top, 60.0, Align::Min);
    let base = num_top + text::baseline_in_box(&ctx, &num_style, 60.0);
    // The OS level of the speaker's endpoint when the OS reports it, else the step's nominal level.
    let unit = match (muted, cx.device.volume_db()) {
        (true, _) => tr("home.muted").to_owned(),
        (false, Some(db)) => fmt::level_db(db),
        (false, None) => fmt::volume_db(vol),
    };
    let unit_c = theme::mix(t.faint, t.a1, motion::toggle(&ctx, Id::new("vol-unit"), muted, 0.2, motion::EASE));
    let ug = text::galley(&ctx, &unit, &unit_style, unit_c);
    let u_ascent = ug.rows.first().and_then(|r| r.glyphs.first()).map_or(10.0, |g| g.pos.y);
    ui.painter().galley(Pos2::new(num_rect.right() + 10.0, base - u_ascent), ug, Color32::WHITE);

    // Strike line: 6 px past each side of the number, rotated −22° and centred on the digits (middle of the
    // figure height, ~0.72 em above the baseline), drawn from the left (scaleX 0 → 1, .28 s spring).
    let strike = motion::toggle(&ctx, Id::new("vol-strike"), muted, 0.28, motion::SPRING);
    if strike > 0.0 {
        let (x0, x1) = (num_rect.left() - 6.0, num_rect.right() + 6.0);
        let centre = Pos2::new((x0 + x1) / 2.0, base - 52.0 * 0.36);
        let dir = Vec2::angled((-22.0f32).to_radians());
        let len = (x1 - x0) / dir.x;
        let a = centre - dir * (len / 2.0);
        ui.painter().add(Shape::line_segment([a, a + dir * len * strike], Stroke::new(2.0_f32, t.a1)));
    }

    let row_mid = top + 16.0 + 35.0;
    let mute = Rect::from_center_size(Pos2::new(group.right() - 18.0, row_mid), Vec2::splat(36.0));
    let tip = tr(if muted { "home.unmute" } else { "home.mute" });
    let icon = if muted { icons::VOLUME_OFF } else { icons::VOLUME_ON };
    if IconButton::new(icon).round().on(muted).tip(tip, None).show(ui, mute, Id::new("mute")).clicked() {
        cx.act(Action::ToggleMute);
    }

    let srect = Rect::from_min_size(Pos2::new(group.left(), top + 16.0 + 70.0), Vec2::new(column.width(), 32.0));
    let mut v = f32::from(vol);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(srect));
    child.set_opacity(1.0 - 0.45 * m);
    if slider::slider(&mut child, srect, Id::new("vol-range"), &mut v, 0.0, 100.0, 1.0, slider::Look::Thin).changed() {
        vol = v.round() as u8;
        cx.act(Action::SetVolume(vol));
    }

    if ctx.pointer_hover_pos().is_some_and(|p| group.contains(p)) && ui.ui_contains_pointer() {
        let scroll = ctx.input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 {
            let nv = (i32::from(vol) + if scroll > 0.0 { 2 } else { -2 }).clamp(0, 100) as u8;
            cx.act(Action::SetVolume(nv));
        }
    }
}
