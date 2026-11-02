mod advanced;
mod device;
mod firmware;
mod legal;
mod nav;
mod outputs;
mod prefs;
mod rows;
mod sound;
mod status;

use eframe::egui::{self, Id, Pos2, Rect, ScrollArea, Vec2};

use super::Cx;
use crate::app::state::Pane;
use crate::theme::motion;

const NAV_W: f32 = 200.0;
const GAP: f32 = 32.0;
const GUTTER: f32 = 12.0;
const END_PAD: f32 = 32.0;

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, content: Rect) {
    let nav_rect = Rect::from_min_max(content.min, Pos2::new(content.left() + NAV_W, content.bottom()));
    nav::show(ui, cx, nav_rect);

    let pane = cx.ui.settings.pane;
    let ctx = ui.ctx().clone();
    let k = motion::SPRING.at(motion::since(&ctx, Id::new("pane-enter"), pane as u64, 0.22));
    // The panes run to the bottom of the window area, under the content padding.
    let area = Rect::from_min_max(Pos2::new(nav_rect.right() + GAP, content.top()), Pos2::new(content.right() + GUTTER, content.bottom() + 32.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area).id_salt(("pane", pane as u8)));
    child.set_opacity(k);
    let visible = content.height();
    // A licence text opened from Legal notices scrolls on its own (from its top).
    let sub = if pane == Pane::Legal { legal::scroll_key(&ctx) } else { 0 };
    ScrollArea::vertical()
        .id_salt(("pane-scroll", pane as u8, sub))
        .max_height(visible)
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            let top = ui.max_rect().top() + 4.0 * (1.0 - k);
            let r = Rect::from_min_size(Pos2::new(ui.max_rect().left(), top), Vec2::new(content.right() - nav_rect.right() - GAP, 1.0));
            let mut p = rows::Page::new(r);
            match pane {
                Pane::Status => status::show(ui, cx, &mut p),
                Pane::Sound => sound::show(ui, cx, &mut p),
                Pane::Outputs => outputs::show(ui, cx, &mut p),
                Pane::Device => device::show(ui, cx, &mut p),
                Pane::Firmware => firmware::show(ui, cx, &mut p),
                Pane::Prefs => prefs::show(ui, cx, &mut p),
                Pane::Advanced => advanced::show(ui, cx, &mut p),
                Pane::Legal => legal::show(ui, cx, &mut p),
            }
            let h = p.y - top;
            let scrolls = h > visible + 0.5;
            ui.allocate_space(Vec2::new(r.width(), h + if scrolls { END_PAD } else { 0.0 }));
        });
}
