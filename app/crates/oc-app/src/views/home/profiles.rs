use eframe::egui::{self, Align, CursorIcon, Id, Pos2, Rect, Sense, Vec2};

use crate::app::actions::Action;
use crate::i18n::{t_with, tr};
use crate::theme::{self, motion, typography::W};
use crate::views::Cx;
use crate::widgets::{text, text::st, tooltip};

const ROW_H: f32 = 40.0;
const ROW_GAP: f32 = 2.0;
pub const LIST_H: f32 = 4.0 * ROW_H + 3.0 * ROW_GAP;

pub fn caption(ui: &egui::Ui, s: &str, x: f32, top: f32) {
    let t = theme::get(ui.ctx());
    text::line(ui.painter(), s, &st::sec_t(), t.muted, x, top, 16.0, Align::Min);
}

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, column: Rect, top: f32) {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    caption(ui, tr("home.profile"), column.left(), top);
    let list = Rect::from_min_size(Pos2::new(column.left(), top + 16.0 + 8.0), Vec2::new(column.width(), LIST_H));
    let active = cx.device.active_profile();
    let profiles: Vec<_> = cx.device.profiles().to_vec();
    for (i, p) in profiles.iter().enumerate() {
        let row = Rect::from_min_size(Pos2::new(list.left(), list.top() + i as f32 * (ROW_H + ROW_GAP)), Vec2::new(list.width(), ROW_H));
        let id = Id::new(("ptab", i));
        let resp = ui.interact(row, id, Sense::click());
        if resp.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.clicked() {
            cx.act(Action::ActivateProfile(i));
        }
        let sel = i == active;
        let hov = resp.hovered();
        let h1 = theme::rgb(p.hue().a);
        let lit = motion::toggle(&ctx, id.with("lit"), sel || hov, 0.25, motion::EASE);
        let k_sel = motion::toggle(&ctx, id.with("sel"), sel, 0.35, motion::SPRING);
        let k_hov = motion::toggle(&ctx, id.with("hov"), hov && !sel, 0.35, motion::SPRING);
        let k_pos = motion::toggle(&ctx, id.with("pos"), sel || hov, 0.25, motion::EASE);

        let w = 16.0 + 6.0 * k_hov + 14.0 * k_sel;
        let h = 2.0 + k_sel;
        let mark = Rect::from_min_size(Pos2::new(row.left(), row.center().y - h / 2.0), Vec2::new(w, h));
        ui.painter().rect_filled(mark, 1.0, h1);

        let style = st::body(if sel { W::Medium } else { W::Regular });
        let name_x = row.left() + 30.0 + 14.0;
        let name = cx.profile_name(i);
        text::line(ui.painter(), &name, &style, theme::mix(t.muted, t.text, lit), name_x, row.top(), ROW_H, Align::Min);

        let pos_c = theme::alpha(theme::mix(t.faint, h1, k_sel), k_pos);
        let pr = text::line(ui.painter(), &(i + 1).to_string(), &st::mono_(11.0, W::Regular), pos_c, row.right(), row.top(), ROW_H, Align::Max);
        if hov {
            tooltip::request(&ctx, pr.expand(4.0), &t_with("home.position", &[("n", &(i + 1))]), None);
        }
    }
    if ctx.pointer_hover_pos().is_some_and(|p| list.contains(p) && p.x < list.right() - 24.0) && ui.ui_contains_pointer() {
        tooltip::request(&ctx, list, tr("home.tip.pod_double_press"), None);
    }
}
