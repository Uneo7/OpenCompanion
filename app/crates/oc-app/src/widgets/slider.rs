use eframe::egui::{self, Color32, CursorIcon, Id, Pos2, Rect, Response, Sense, Vec2};

use super::paint;
use crate::theme::{self, motion};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Settings,
    Thin,
}

impl Look {
    fn thumb(self) -> f32 {
        match self {
            Look::Settings => 18.0,
            Look::Thin => 14.0,
        }
    }

    fn track(self) -> f32 {
        match self {
            Look::Settings => 4.0,
            Look::Thin => 2.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Opts {
    pub accent: Option<Color32>,
    pub ring: bool,
    pub disabled: bool,
    /// Default value restored by a double-click (`None`: no reset, e.g. the master volume).
    pub reset: Option<f32>,
    /// Shows a value set elsewhere (the bass module's rear knob): same size, muted to 72 %, no hover, drag or
    /// pointer cursor.
    pub read_only: bool,
}

#[derive(Clone, Copy)]
struct Glide {
    from: f32,
    start: f64,
}

const GLIDE_SECS: f32 = 0.25;

/// Value ↔ position, WebKit style: the thumb centre travels between half a thumb from each end.
fn ratio(v: f32, min: f32, max: f32) -> f32 {
    ((v - min) / (max - min)).clamp(0.0, 1.0)
}

/// Slider in `rect` (the input box: the track is vertically centred). `step` snaps the value.
#[allow(clippy::too_many_arguments)]
pub fn slider(ui: &mut egui::Ui, rect: Rect, id: Id, value: &mut f32, min: f32, max: f32, step: f32, look: Look) -> Response {
    slider_ex(ui, rect, id, value, (min, max, step), look, Opts::default())
}

pub fn slider_ex(ui: &mut egui::Ui, rect: Rect, id: Id, value: &mut f32, range: (f32, f32, f32), look: Look, o: Opts) -> Response {
    let (min, max, step) = range;
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let o = Opts { disabled: o.disabled || o.read_only, ..o };
    let sense = if o.disabled { Sense::hover() } else { Sense::click_and_drag() };
    let mut resp = ui.interact(rect, id, sense);
    let th = look.thumb();
    if let Some(p) = resp.interact_pointer_pos().filter(|_| !o.disabled && (resp.dragged() || resp.clicked() || resp.is_pointer_button_down_on())) {
        let k = ((p.x - rect.left() - th / 2.0) / (rect.width() - th)).clamp(0.0, 1.0);
        let raw = min + k * (max - min);
        let snapped = ((raw - min) / step).round() * step + min;
        let v = snapped.clamp(min, max);
        if (v - *value).abs() > f32::EPSILON {
            *value = v;
            resp.mark_changed();
        }
    }
    if let Some(d) = o.reset.filter(|_| !o.disabled && resp.double_clicked()) {
        let from = ratio(*value, min, max);
        let d = d.clamp(min, max);
        if (d - *value).abs() > f32::EPSILON {
            *value = d;
            resp.mark_changed();
        }
        if !motion::reduced(&ctx) {
            let start = ctx.input(|i| i.time);
            ctx.data_mut(|m| m.insert_temp(id.with("glide"), Glide { from, start }));
        }
    }
    if !o.disabled && (resp.hovered() || resp.dragged()) {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
    let mut p = ratio(*value, min, max);
    if let Some(g) = ctx.data(|m| m.get_temp::<Glide>(id.with("glide"))) {
        let k = ((ctx.input(|i| i.time) - g.start) as f32 / GLIDE_SECS).clamp(0.0, 1.0);
        if k < 1.0 && !resp.dragged() {
            p = g.from + (p - g.from) * motion::SPRING.at(k);
            ctx.request_repaint();
        } else {
            ctx.data_mut(|m| m.remove::<Glide>(id.with("glide")));
        }
    }
    let painter = paint::faded(ui.painter(), if o.read_only { 0.72 } else if o.disabled { t.disabled } else { 1.0 });
    let accent = o.accent.unwrap_or(t.fill);
    let hover = motion::toggle(&ctx, id.with("h"), !o.disabled && resp.hovered(), 0.12, motion::EASE);
    let active = motion::toggle(&ctx, id.with("a"), !o.disabled && (resp.dragged() || resp.is_pointer_button_down_on()), 0.12, motion::EASE);
    let cy = rect.center().y;
    let tw = look.track();
    let track = Rect::from_min_max(Pos2::new(rect.left(), cy - tw / 2.0), Pos2::new(rect.right(), cy + tw / 2.0));
    painter.rect_filled(track, tw / 2.0, t.track);
    // `--p` is the value ratio of the whole input box (the fill runs under the thumb).
    let fill = Rect::from_min_max(track.min, Pos2::new(track.left() + track.width() * p, track.bottom()));
    if fill.width() > 0.5 {
        painter.rect_filled(fill, tw / 2.0, accent);
    }
    let c = Pos2::new(rect.left() + th / 2.0 + p * (rect.width() - th), cy);
    let scale = 1.0 + 0.1 * hover + 0.08 * active;
    let r = th / 2.0 * scale;
    painter.circle_filled(c + Vec2::new(0.0, 1.0), r + 1.0, theme::alpha(t.thumb_shadow, 0.55));
    if o.ring {
        painter.circle_filled(c, r + 2.0, accent);
    }
    painter.circle_filled(c, r, t.thumb);
    resp
}

