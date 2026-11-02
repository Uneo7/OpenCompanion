use eframe::egui::{self, Color32, FontId, Id, Margin, Rect, Response, Stroke, StrokeKind, TextEdit};

use crate::theme::{self, motion};

pub fn field(ui: &mut egui::Ui, rect: Rect, id: Id, value: &mut String, placeholder: &str, font: FontId, max_len: Option<usize>) -> Response {
    let t = theme::get(ui.ctx());
    ui.painter().rect_filled(rect, 8.0, t.field);
    let row_h = ui.ctx().fonts(|f| f.row_height(&font));
    let pad_y = ((rect.height() - row_h) / 2.0).max(0.0);
    let mut edit = TextEdit::singleline(value)
        .id(id)
        .frame(false)
        .font(font)
        .text_color(t.text)
        .margin(Margin { left: 12, right: 12, top: pad_y as i8, bottom: 0 })
        .hint_text(egui::RichText::new(placeholder).color(t.muted))
        .desired_width(rect.width());
    if let Some(n) = max_len {
        edit = edit.char_limit(n);
    }
    let resp = ui.scope(|ui| {
        style_text_edit(ui, t.a1, t.text);
        ui.put(rect, edit)
    });
    let resp = resp.inner;
    let k = motion::toggle(ui.ctx(), id.with("focus"), resp.has_focus(), 0.15, motion::EASE);
    if k > 0.0 {
        glow(ui.painter(), rect, 8.0, k);
    }
    resp
}

/// The soft focus ring (`--glow`: 3 px of the accent at about 24 %).
pub fn glow(painter: &egui::Painter, rect: Rect, radius: f32, k: f32) {
    let t = theme::get(painter.ctx());
    painter.rect_stroke(rect.expand(1.5), radius + 1.5, Stroke::new(3.0_f32, theme::alpha(t.glow_ring, k)), StrokeKind::Middle);
}

pub fn style_text_edit(ui: &mut egui::Ui, accent: Color32, text: Color32) {
    let v = ui.visuals_mut();
    v.selection.bg_fill = theme::alpha(accent, 0.35);
    v.selection.stroke = Stroke::new(1.0_f32, text);
    v.text_cursor.stroke = Stroke::new(1.5_f32, text);
    v.extreme_bg_color = Color32::TRANSPARENT;
}
