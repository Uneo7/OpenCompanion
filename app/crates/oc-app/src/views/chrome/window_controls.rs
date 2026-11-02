//! No maximise: the window has one fixed size.

use eframe::egui::{self, Color32, CornerRadius, CursorIcon, Id, Pos2, Rect, Sense, Vec2};

use crate::app::actions::Action;
use crate::assets::icons::{self, Icon};
use crate::theme::{self, motion};
use crate::views::Cx;

/// Compact buttons (hit area), inset from the window's edge (rounded corners on Windows 11 and macOS).
const W: f32 = 36.0;
const H: f32 = 30.0;
const GAP: f32 = 4.0;
/// Inset from the window's right edge, and how far below the bar's centre (away from the top edge).
pub const INSET: f32 = 12.0;
pub const DOWN: f32 = 2.0;

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, bar: Rect) -> f32 {
    let cy = bar.center().y + DOWN;
    let close = Rect::from_min_size(Pos2::new(bar.right() - INSET - W, cy - H / 2.0), Vec2::new(W, H));
    let min = close.translate(Vec2::new(-W - GAP, 0.0));
    if button(ui, min, Id::new("win-min"), icons::MINIMIZE, false) {
        cx.act(Action::Minimize);
    }
    if button(ui, close, Id::new("win-close"), icons::CLOSE, true) {
        cx.act(Action::AskClose);
    }
    min.left()
}

fn button(ui: &mut egui::Ui, rect: Rect, id: Id, icon: Icon, is_close: bool) -> bool {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let resp = ui.interact(rect, id, Sense::click());
    if resp.hovered() {
        ctx.set_cursor_icon(CursorIcon::Default);
    }
    let h = motion::toggle(&ctx, id.with("h"), resp.hovered(), 0.1, motion::EASE);
    let down = resp.is_pointer_button_down_on();
    let (bg, fg) = if is_close {
        let red = if down { Color32::from_rgb(0xb3, 0x26, 0x1a) } else { Color32::from_rgb(0xc4, 0x2b, 0x1c) };
        (theme::alpha(red, h), theme::mix(theme::alpha(t.text, 0.6), Color32::WHITE, h))
    } else {
        let bg = if down { t.hover } else { t.raised };
        (theme::alpha(bg, h), theme::mix(theme::alpha(t.text, 0.6), t.text, h))
    };
    ui.painter().rect_filled(rect, CornerRadius::same(8), bg);
    icon.paint(ui.painter(), rect.center(), 14.0, fg);
    resp.clicked()
}
