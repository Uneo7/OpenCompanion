use eframe::egui::{self, Align, CursorIcon, Id, Pos2, Rect, Sense, Vec2};

use super::{paint, text, text::st};
use crate::theme::{self, motion, typography::W};

const GAP: f32 = 22.0;
const H: f32 = 32.0;
const PAD: f32 = 2.0;

fn style() -> text::Style {
    st::inter_(13.5, W::Medium).tracking(0.005)
}

/// Tabs centred on `center`; returns the index clicked this frame. `current` = None: no tab is current (the
/// mark stays where it was). `locked` dims the tabs and ignores clicks (installer running).
pub fn text_tabs(ui: &mut egui::Ui, center: Pos2, id: Id, tabs: &[&str], current: Option<usize>, locked: bool) -> Option<usize> {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let widths: Vec<f32> = tabs.iter().map(|tab| text::width(&ctx, tab, &style()) + 2.0 * PAD).collect();
    let total = widths.iter().sum::<f32>() + GAP * (tabs.len() as f32 - 1.0);
    let lock_k = motion::toggle(&ctx, id.with("locked"), locked, 0.2, motion::EASE);
    let painter = paint::faded(ui.painter(), 1.0 - 0.55 * lock_k);
    let mut x = center.x - total / 2.0;
    let mut clicked = None;
    let mut mark_x = None;
    for (i, tab) in tabs.iter().enumerate() {
        let rect = Rect::from_min_size(Pos2::new(x, center.y - H / 2.0), Vec2::new(widths[i], H));
        let tid = id.with(i);
        let on = current == Some(i);
        let resp = ui.interact(rect, tid, if locked { Sense::hover() } else { Sense::click() });
        if resp.clicked() {
            clicked = Some(i);
        }
        if resp.hovered() && !locked {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        let h = motion::toggle(&ctx, tid.with("lit"), on || (resp.hovered() && !locked), 0.2, motion::EASE);
        text::line(&painter, tab, &style(), theme::mix(t.muted, t.text, h), rect.left() + PAD, rect.top(), H, Align::Min);
        if on {
            mark_x = Some(rect.center().x);
        }
        x += widths[i] + GAP;
    }
    // The mark keeps its last place when no tab is current.
    let key = id.with("mark-target");
    let target = mark_x.or_else(|| ctx.data(|d| d.get_temp::<f32>(key)));
    if let Some(tx) = target {
        ctx.data_mut(|d| d.insert_temp(key, tx));
        let mx = motion::tween(&ctx, id.with("mark"), tx, 0.32, motion::SPRING);
        let mark = Rect::from_min_size(Pos2::new(mx - 7.0, center.y + H / 2.0 - 2.0 - 2.0), Vec2::new(14.0, 2.0));
        painter.rect_filled(mark, 1.0, t.a1);
    }
    clicked
}
