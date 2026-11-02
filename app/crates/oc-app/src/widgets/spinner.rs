//! Small spinner: a 270° arc in the profile colour turning once a second.

use eframe::egui::{self, Pos2, Shape, Stroke, Vec2};

pub fn spinner(ui: &egui::Ui, c: Pos2, r: f32) {
    let t = crate::theme::get(ui.ctx());
    let a0 = (ui.ctx().input(|i| i.time) as f32 * std::f32::consts::TAU) % std::f32::consts::TAU;
    let pts: Vec<Pos2> = (0..=24).map(|k| a0 + k as f32 / 24.0 * 1.5 * std::f32::consts::PI).map(|a| c + Vec2::angled(a) * r).collect();
    ui.painter().add(Shape::line(pts, Stroke::new(2.0_f32, t.a1)));
    ui.ctx().request_repaint();
}
