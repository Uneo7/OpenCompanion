mod values;

use eframe::egui::{self, Align, CursorIcon, Id, Pos2, Rect, Sense, Vec2};

use super::menus;
use crate::assets::icons::{self, Icon};
use crate::i18n::{t_with, tr};
use crate::model::eq::Band;
use crate::theme::{self, motion, typography::W};
use crate::views::Cx;
use crate::widgets::{fmt, menu, menu::Anchor, text, text::st, tooltip};

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, panel: Rect) {
    let ctx = ui.ctx().clone();
    let key = super::target_index(cx.ui.eq.target);
    let k = motion::SPRING.at(motion::since(&ctx, Id::new("bp-enter"), key, 0.22));
    let top = panel.top() + 8.0 + 4.0 * (1.0 - k);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(panel.expand(2.0)).id_salt("band-panel"));
    child.set_opacity(k);
    let ui = &mut child;
    let eq = &cx.ui.eq;
    if eq.compare {
        return empty(ui, panel, top, icons::EAR, tr("eq.abmsg"));
    }
    let sel = eq.selected;
    let target = eq.target;
    let Some(band) = eq.draft.get(target).get(sel).copied() else {
        let msg = t_with("eq.empty", &[("target", &tr(fmt::target_key(target)))]);
        return empty(ui, panel, top, icons::EQ, &msg);
    };
    let t = theme::get(&ctx);
    let hue = t.band(sel);
    top_row(ui, cx, panel.left(), top, sel, &band, hue);
    values::show(ui, cx, panel, top + 34.0 + 12.0, sel, band, hue);
}

fn empty(ui: &mut egui::Ui, panel: Rect, top: f32, icon: Icon, msg: &str) {
    let t = theme::get(ui.ctx());
    let cy = (panel.top() + panel.bottom() - 32.0) / 2.0 + (top - panel.top() - 8.0);
    let style = st::body(W::Regular);
    let line_w = text::width(ui.ctx(), msg, &style);
    let one_line = 20.0 + 12.0 + line_w <= panel.width();
    let left = if one_line { panel.center().x - (20.0 + 12.0 + line_w) / 2.0 } else { panel.left() };
    icon.paint(ui.painter(), Pos2::new(left + 10.0, cy), 20.0, t.muted);
    let x = left + 20.0 + 12.0;
    let w = if one_line { line_w + 1.0 } else { panel.right() - x };
    let h = text::paragraph_height(ui.ctx(), msg, &style, w, 20.0);
    text::paragraph(ui.painter(), msg, &style, t.muted, Pos2::new(x, cy - h / 2.0), w, 20.0);
}

fn top_row(ui: &mut egui::Ui, cx: &mut Cx, left: f32, top: f32, sel: usize, band: &Band, hue: eframe::egui::Color32) {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let cy = top + 17.0;
    ui.painter().circle_filled(Pos2::new(left + 4.0, cy), 4.0, hue);
    let id_rect = text::line(ui.painter(), &t_with("eq.band", &[("n", &(sel + 1))]), &st::body(W::Medium), t.text, left + 16.0, top, 34.0, Align::Min);

    let label = fmt::tr_type(band.kind);
    let style = st::body(W::Regular);
    let w = 8.0 + text::width(&ctx, label, &style) + 8.0 + 14.0 + 8.0;
    let r = Rect::from_min_size(Pos2::new(id_rect.right() + 12.0, cy - 18.0), Vec2::new(w, 36.0));
    let key = Id::new("band-type");
    let open = cx.menu_open(key);
    let resp = ui.interact(r, key.with("btn"), Sense::click());
    if resp.hovered() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
    if !open {
        tooltip::attach(&resp, &fmt::type_help(band.kind), None);
    }
    let h = motion::hover(&ctx, key, resp.hovered() || open);
    ui.painter().rect_filled(r, 8.0, theme::alpha(t.field, h));
    let lr = text::line(ui.painter(), label, &style, theme::mix(t.muted, t.text, h), r.left() + 8.0, r.top(), 36.0, Align::Min);
    menu::chevron(ui, Pos2::new(lr.right() + 8.0 + 7.0, r.center().y), key.with("chev"), open);
    if resp.clicked() {
        cx.toggle_menu(key, Anchor::Rect(r), false, menus::type_menu(sel, band));
    }
}
