use eframe::egui::{self, Align, CursorIcon, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::app::{actions::Action, state::Pane};
use crate::assets::icons::{self, Icon};
use crate::i18n::tr;
use crate::theme::{self, motion, typography::W};
use crate::views::Cx;
use crate::widgets::{text, text::st};

fn entry(p: Pane) -> (Icon, &'static str) {
    match p {
        Pane::Status => (icons::PULSE, "settings.nav.status"),
        Pane::Sound => (icons::EQ, "settings.nav.sound"),
        Pane::Outputs => (icons::SPEAKER, "settings.nav.outputs"),
        Pane::Device => (icons::SPEAKERS, "settings.nav.device"),
        Pane::Firmware => (icons::CHIP, "settings.firmware"),
        Pane::Prefs => (icons::MONITOR, "settings.prefs.title"),
        Pane::Advanced => (icons::TERMINAL, "settings.nav.advanced"),
        Pane::Legal => (icons::INFO, "settings.nav.legal"),
    }
}

const ROW_H: f32 = 36.0;

fn rows(rect: Rect) -> Vec<(Pane, Rect)> {
    let mut y = rect.top() + 32.0 + 24.0;
    Pane::ALL
        .iter()
        .map(|p| {
            if *p == Pane::Advanced {
                y += 12.0;
            }
            let r = Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(rect.width(), ROW_H));
            y += ROW_H + 2.0;
            (*p, r)
        })
        .collect()
}

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, rect: Rect) {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    text::line(ui.painter(), tr("settings.title"), &st::page(), t.text, rect.left(), rect.top(), 32.0, Align::Min);
    let rows = rows(rect);

    let current = cx.ui.settings.pane;
    if let Some((_, r)) = rows.iter().find(|(p, _)| *p == current) {
        let y = rect.top() + motion::tween(&ctx, Id::new("subnav-ind"), r.top() - rect.top(), 0.26, motion::SPRING);
        let ind = Rect::from_min_size(Pos2::new(r.left(), y), r.size());
        ui.painter().rect_filled(ind, 8.0, t.raised);
        ui.painter().rect_stroke(ind, 8.0, Stroke::new(1.0_f32, t.line), StrokeKind::Inside);
    }

    for (p, r) in rows {
        if p == Pane::Advanced {
            let ly = (r.top() - 7.0).round() + 0.5;
            ui.painter().line_segment([Pos2::new(rect.left() + 10.0, ly), Pos2::new(rect.right() - 10.0, ly)], Stroke::new(1.0_f32, t.line));
        }
        let id = Id::new(("subnav", p as u8));
        let resp = ui.interact(r, id, Sense::click());
        if resp.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.clicked() {
            // Leaving the pane, or clicking it again, closes an open licence text.
            super::legal::close(&ctx);
            if p != current {
                cx.act(Action::ShowPane(p));
            }
        }
        let on = current == p;
        let k = motion::toggle(&ctx, id.with("on"), on, 0.15, motion::EASE);
        let h = motion::hover(&ctx, id, resp.hovered() && !on);
        if h > 0.0 {
            ui.painter().rect_filled(r, 8.0, theme::alpha(t.surface, h));
        }
        let (icon, key) = entry(p);
        icon.paint(ui.painter(), Pos2::new(r.left() + 10.0 + 9.0, r.center().y), 18.0, theme::mix(t.muted, t.a1, k));
        text::line(ui.painter(), tr(key), &st::body(W::Medium), t.text, r.left() + 10.0 + 18.0 + 12.0, r.top(), ROW_H, Align::Min);
    }
}
