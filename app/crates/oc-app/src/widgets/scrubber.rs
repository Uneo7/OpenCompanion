use eframe::egui::{
    self,
    text::{CCursor, CCursorRange},
    Align, Color32, CursorIcon, Id, Key, Margin, Rect, Sense, TextEdit,
};

use super::{input, text, text::st, tooltip};
use crate::model::format;
use crate::theme::{self, motion, typography::W};

#[derive(Clone, Copy)]
struct Drag {
    x0: f32,
    v0: f64,
    moved: bool,
}

pub struct Scrubber<'a> {
    pub field: Rect,
    pub value_box: Rect,
    pub value: f64,
    pub display: &'a str,
    pub unit: &'a str,
    /// Step at the current value (frequency steps grow with the frequency).
    pub step: f64,
    pub enabled: bool,
    pub color: Color32,
    pub tip: &'a str,
    pub default: Option<f64>,
}

pub fn value_style() -> text::Style {
    st::inter_(30.0, W::SemiBold).tracking(-0.035)
}

pub fn scrubber(ui: &mut egui::Ui, id: Id, s: Scrubber) -> Option<f64> {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let edit_id = id.with("edit");
    let editing: Option<String> = ctx.data(|d| d.get_temp(edit_id));
    let over_field = s.enabled && ctx.pointer_hover_pos().is_some_and(|p| s.field.contains(p)) && ui.ui_contains_pointer();
    let h = motion::hover(&ctx, id, over_field || editing.is_some());
    if h > 0.0 {
        ui.painter().rect_filled(s.field, 12.0, theme::alpha(t.field, h));
    }
    let sense = if s.enabled && editing.is_none() { Sense::click_and_drag() } else { Sense::hover() };
    let resp = ui.interact(s.value_box, id, sense);
    let mut out = None;

    // Double-click on the value: back to the default. The first click has opened the text field, so the second
    // one lands there: catch it on the value box and close the field.
    let dbl = s.enabled
        && ctx.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary))
        && ctx.pointer_interact_pos().is_some_and(|p| s.value_box.contains(p));
    if let Some(d) = s.default.filter(|_| dbl) {
        ctx.data_mut(|m| m.remove::<String>(edit_id));
        ctx.memory_mut(|m| m.surrender_focus(edit_id.with("te")));
        return Some(d);
    }

    if let Some(mut buf) = editing {
        input::glow(ui.painter(), s.field, 12.0, 1.0);
        let style = value_style();
        let font = style.font.clone();
        let row_h = ctx.fonts(|f| f.row_height(&font));
        let te = TextEdit::singleline(&mut buf)
            .id(edit_id.with("te"))
            .frame(false)
            .font(font)
            .text_color(s.color)
            .margin(Margin { left: 0, right: 0, top: ((s.value_box.height() - row_h) / 2.0) as i8, bottom: 0 })
            .desired_width(s.value_box.width());
        let r = ui
            .scope(|ui| {
                input::style_text_edit(ui, t.a1, t.text);
                ui.put(s.value_box, te)
            })
            .inner;
        if !r.has_focus() && !r.lost_focus() {
            r.request_focus();
            if let Some(mut st) = TextEdit::load_state(&ctx, edit_id.with("te")) {
                st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(buf.chars().count()))));
                st.store(&ctx, edit_id.with("te"));
            }
        }
        let cancel = ctx.input(|i| i.key_pressed(Key::Escape));
        if r.lost_focus() || cancel {
            if !cancel {
                out = format::parse_num(&buf);
            }
            ctx.data_mut(|d| d.remove::<String>(edit_id));
        } else {
            ctx.data_mut(|d| d.insert_temp(edit_id, buf));
        }
        return out;
    }

    if s.enabled {
        if resp.hovered() || resp.dragged() {
            ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
        tooltip::attach(&resp, s.tip, None);
        if resp.drag_started() {
            let x0 = resp.interact_pointer_pos().map_or(0.0, |p| p.x);
            ctx.data_mut(|d| d.insert_temp(id.with("drag"), Drag { x0, v0: s.value, moved: false }));
        }
        if resp.dragged() {
            if let (Some(mut d), Some(p)) = (ctx.data(|d| d.get_temp::<Drag>(id.with("drag"))), resp.interact_pointer_pos()) {
                let dx = p.x - d.x0;
                if dx.abs() > 2.0 {
                    d.moved = true;
                }
                if d.moved {
                    out = Some(d.v0 + f64::from((dx / 3.0).round()) * s.step);
                }
                ctx.data_mut(|m| m.insert_temp(id.with("drag"), d));
            }
        }
        if resp.clicked() {
            ctx.data_mut(|d| d.insert_temp(edit_id, s.display.to_owned()));
            ctx.request_repaint();
        }
        if resp.hovered() {
            let scroll = ctx.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                let fine = ctx.input(|i| i.modifiers.shift);
                let k = if fine { 1.0 } else { 3.0 };
                out = Some(s.value + scroll.signum() as f64 * s.step * k);
            }
        }
    }
    let color = if s.enabled { s.color } else { t.muted };
    let vs = value_style();
    let r = text::line(ui.painter(), s.display, &vs, color, s.value_box.left(), s.value_box.top(), s.value_box.height(), Align::Min);
    if !s.unit.is_empty() {
        let us = st::mono_(10.0, W::Regular);
        let base = s.value_box.top() + text::baseline_in_box(&ctx, &vs, s.value_box.height());
        let ut = base - text::baseline_in_box(&ctx, &us, 14.0);
        text::line(ui.painter(), s.unit, &us, t.faint, r.right() + 5.0, ut, 14.0, Align::Min);
    }
    out
}
