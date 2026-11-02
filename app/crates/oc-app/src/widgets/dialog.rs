use eframe::egui::{
    Align, Area, Color32, Context, CursorIcon, Id, Key, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};

use super::{button, input, paint, text, text::st};
use crate::theme::{self, motion, typography::W};

pub const WIDTH: f32 = 400.0;
/// The write plan needs room for its addresses.
const PLAN_WIDTH: f32 = 560.0;

fn width(body: &Body) -> f32 {
    if matches!(body, Body::Plan { .. }) { PLAN_WIDTH } else { WIDTH }
}

#[derive(Clone, PartialEq)]
pub struct PlanLine {
    pub name: String,
    pub detail: String,
}

#[derive(Clone, PartialEq)]
pub enum Body {
    Text(String),
    Field { value: String, max_len: usize },
    /// Paragraph + text field that must match a word (dangerous confirmations).
    Confirm { text: String, placeholder: String, value: String },
    /// Two-colour swatches (profile colour). `taken[i]`: the colour is another profile's (its name): shown
    /// dimmed with a lock, not selectable, the tooltip says whose it is.
    Hues { hues: Vec<(Color32, Color32)>, selected: usize, taken: Vec<Option<String>> },
    Plan { sub: String, lines: Vec<PlanLine>, warning: Option<String> },
}

const PLAN_ROW_PAD: f32 = 10.0;
const PLAN_DOT: f32 = 16.0;
const PLAN_WARN_PAD: f32 = 14.0;

fn plan_row_height(ctx: &Context, l: &PlanLine, w: f32) -> f32 {
    let name = text::paragraph_height(ctx, &l.name, &st::body(W::Medium), w - PLAN_DOT, 20.0);
    let detail = if l.detail.is_empty() { 0.0 } else { 2.0 + text::paragraph_height(ctx, &l.detail, &st::mono_(12.0, W::Regular), w - PLAN_DOT, 18.0) };
    PLAN_ROW_PAD + name + detail + PLAN_ROW_PAD
}

fn plan_warning_height(ctx: &Context, s: &str, w: f32) -> f32 {
    PLAN_WARN_PAD + text::paragraph_height(ctx, s, &st::inter_(13.0, W::SemiBold), w - 2.0 * PLAN_WARN_PAD - 26.0, 19.0) + PLAN_WARN_PAD
}

#[derive(Clone)]
pub struct Dialog<A> {
    pub title: String,
    pub body: Body,
    pub ok: String,
    pub danger: bool,
    pub action: A,
    pub serial: u64,
}

pub enum Outcome {
    Stay,
    Cancel,
    Ok,
}

fn body_height(ctx: &Context, body: &Body, inner_w: f32) -> f32 {
    match body {
        Body::Text(s) => text::paragraph_height(ctx, s, &st::body(W::Regular), inner_w, 20.0),
        Body::Field { .. } => 36.0,
        Body::Confirm { text: s, .. } => text::paragraph_height(ctx, s, &st::body(W::Regular), inner_w, 20.0) + 12.0 + 36.0,
        Body::Hues { hues, .. } => {
            let per_row = ((inner_w + 10.0) / 44.0).floor().max(1.0) as usize;
            let rows = hues.len().div_ceil(per_row);
            rows as f32 * 34.0 + (rows.saturating_sub(1)) as f32 * 10.0
        }
        Body::Plan { sub, lines, warning } => {
            let sub_h = if sub.is_empty() { 0.0 } else { 18.0 + 14.0 };
            let rows: f32 = lines.iter().map(|l| plan_row_height(ctx, l, inner_w)).sum::<f32>() + 1.0;
            let warn = warning.as_ref().map_or(0.0, |w| 20.0 + plan_warning_height(ctx, w, inner_w));
            sub_h + rows + warn + 8.0
        }
    }
}

/// Shows `dlg` (`k` = entrance progress 0–1; `closing` makes it inert). Edits the body in place.
pub fn show<A>(ctx: &Context, dlg: &mut Dialog<A>, window: Rect, k: f32, closing: bool) -> Outcome {
    let t = theme::get(ctx);
    // Ids carry the serial: a dialog fading out and a new one never share widgets.
    let base = Id::new(("oc-dialog", dlg.serial));
    let mut outcome = Outcome::Stay;
    let w = width(&dlg.body);
    let inner_w = w - 2.0 * 24.0;
    let h = 24.0 + 24.0 + 12.0 + body_height(ctx, &dlg.body, inner_w) + 12.0 + 12.0 + 36.0 + 24.0 + 2.0;
    // translate(-50%, -47%) → (-50%, -50%)
    let y_shift = 0.03 * h * (1.0 - k);
    let rect = Rect::from_center_size(window.center() + Vec2::new(0.0, y_shift), Vec2::new(w, h));

    // Scrim: blocks the window and closes the dialog when clicked.
    Area::new(base.with("scrim"))
        .order(Order::Foreground)
        .fixed_pos(window.min)
        .constrain(false)
        .show(ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(window.size(), Sense::click());
            ui.painter().rect_filled(r, theme::metrics::window_radius(), theme::alpha(t.scrim, k));
            if resp.clicked() && !closing && !rect.contains(resp.interact_pointer_pos().unwrap_or_default()) {
                outcome = Outcome::Cancel;
            }
        });

    Area::new(base.with("panel"))
        .order(Order::Foreground)
        .fixed_pos(rect.min)
        .constrain(false)
        .show(ctx, |ui| {
            ui.set_opacity(k);
            let (r, _) = ui.allocate_exact_size(rect.size(), Sense::click());
            let painter = ui.painter().clone();
            paint::shadow(&painter, r, 16.0, 20.0, 60.0, 0.0, t.dialog_shadow);
            painter.rect(r, 16.0, t.pop, Stroke::new(1.0_f32, t.pop_edge), StrokeKind::Inside);
            let left = r.left() + 1.0 + 24.0;
            let mut y = r.top() + 1.0 + 24.0;
            let title = st::inter_(18.0, W::SemiBold).tracking(-0.015);
            text::line(&painter, &dlg.title, &title, t.text, left, y, 24.0, Align::Min);
            y += 24.0 + 12.0;
            let body_style = st::body(W::Regular);
            let mut enter = false;
            match &mut dlg.body {
                Body::Text(s) => {
                    y += text::paragraph(&painter, s, &body_style, t.muted, Pos2::new(left, y), inner_w, 20.0);
                }
                Body::Field { value, max_len } => {
                    let fr = Rect::from_min_size(Pos2::new(left, y), Vec2::new(inner_w, 36.0));
                    let resp = input::field(ui, fr, base.with("field"), value, "", st::body(W::Regular).font, Some(*max_len));
                    focus_once(ctx, &resp, dlg.serial);
                    enter = resp.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter));
                    y += 36.0;
                }
                Body::Confirm { text: s, placeholder, value } => {
                    y += text::paragraph(&painter, s, &body_style, t.muted, Pos2::new(left, y), inner_w, 20.0) + 12.0;
                    let fr = Rect::from_min_size(Pos2::new(left, y), Vec2::new(inner_w, 36.0));
                    let resp = input::field(ui, fr, base.with("confirm"), value, placeholder, st::body(W::Regular).font, None);
                    focus_once(ctx, &resp, dlg.serial);
                    enter = resp.lost_focus() && ctx.input(|i| i.key_pressed(Key::Enter));
                    y += 36.0;
                }
                Body::Hues { hues, selected, taken } => {
                    let per_row = ((inner_w + 10.0) / 44.0).floor().max(1.0) as usize;
                    for (i, (h1, h2)) in hues.iter().enumerate() {
                        let c = Pos2::new(left + 17.0 + (i % per_row) as f32 * 44.0, y + 17.0 + (i / per_row) as f32 * 44.0);
                        let sr = Rect::from_center_size(c, Vec2::splat(34.0));
                        let owner = taken.get(i).and_then(|o| o.as_deref()).filter(|_| *selected != i);
                        let resp = ui.interact(sr, base.with(("hue", i)), if owner.is_some() { Sense::hover() } else { Sense::click() });
                        if let Some(name) = owner {
                            super::tooltip::attach_top(&resp, &crate::i18n::t_with("profile.color.taken", &[("name", &name)]));
                        } else if resp.hovered() {
                            ctx.set_cursor_icon(CursorIcon::PointingHand);
                        }
                        if resp.clicked() && owner.is_none() {
                            *selected = i;
                        }
                        if *selected == i {
                            painter.circle_filled(c, 21.0, t.text);
                            painter.circle_filled(c, 19.0, t.bg);
                        }
                        if owner.is_some() {
                            // Same hues, lower opacity, and a small lock: used by another profile.
                            paint::split_circle(&painter, c, 17.0, theme::alpha(*h1, 0.35), theme::alpha(*h2, 0.35));
                            crate::assets::icons::LOCK.paint(&painter, c, 14.0, t.text);
                        } else {
                            paint::split_circle(&painter, c, 17.0, *h1, *h2);
                        }
                    }
                    y += body_height(ctx, &dlg.body, inner_w);
                }
                Body::Plan { sub, lines, warning } => {
                    if !sub.is_empty() {
                        text::line(&painter, sub, &st::mono_(12.0, W::Regular), t.muted, left, y, 18.0, Align::Min);
                        y += 18.0 + 14.0;
                    }
                    let rule = |y: f32| painter.line_segment([Pos2::new(left, y + 0.5), Pos2::new(left + inner_w, y + 0.5)], Stroke::new(1.0_f32, t.sep));
                    rule(y);
                    for l in lines.iter() {
                        let rh = plan_row_height(ctx, l, inner_w);
                        painter.circle_filled(Pos2::new(left + 4.0, y + PLAN_ROW_PAD + 10.0), 3.5, t.a1);
                        let x = left + PLAN_DOT;
                        let mut ly = y + PLAN_ROW_PAD;
                        ly += text::paragraph(&painter, &l.name, &st::body(W::Medium), t.text, Pos2::new(x, ly), inner_w - PLAN_DOT, 20.0);
                        if !l.detail.is_empty() {
                            text::paragraph(&painter, &l.detail, &st::mono_(12.0, W::Regular), t.muted, Pos2::new(x, ly + 2.0), inner_w - PLAN_DOT, 18.0);
                        }
                        y += rh;
                        rule(y);
                    }
                    y += 1.0;
                    if let Some(wtext) = warning {
                        y += 20.0;
                        let wh = plan_warning_height(ctx, wtext, inner_w);
                        let wr = Rect::from_min_size(Pos2::new(left, y), Vec2::new(inner_w, wh));
                        painter.rect_filled(wr, 12.0, t.danger_fill);
                        crate::assets::icons::WARN.paint(&painter, Pos2::new(wr.left() + PLAN_WARN_PAD + 9.0, wr.top() + PLAN_WARN_PAD + 9.5), 18.0, t.on_danger);
                        let tx = wr.left() + PLAN_WARN_PAD + 26.0;
                        text::paragraph(&painter, wtext, &st::inter_(13.0, W::SemiBold), t.on_danger, Pos2::new(tx, wr.top() + PLAN_WARN_PAD), inner_w - 2.0 * PLAN_WARN_PAD - 26.0, 19.0);
                        y += wh;
                    }
                    y += 8.0;
                }
            }
            y += 12.0 + 12.0;
            let ok = button::Button::new(&dlg.ok).kind(if dlg.danger { button::Kind::Danger } else { button::Kind::Primary });
            let ok_w = ok.size(ctx).x;
            let ok_min = Pos2::new(r.right() - 1.0 - 24.0 - ok_w, y);
            let cancel = button::Button::new(crate::i18n::tr("dialog.cancel")).kind(button::Kind::Quiet);
            let cancel_w = cancel.size(ctx).x;
            if !closing {
                if cancel.show(ui, Pos2::new(ok_min.x - 8.0 - cancel_w, y), base.with("cancel")).clicked() {
                    outcome = Outcome::Cancel;
                }
                if ok.show(ui, ok_min, base.with("ok")).clicked() || enter {
                    outcome = Outcome::Ok;
                }
            } else {
                cancel.show(ui, Pos2::new(ok_min.x - 8.0 - cancel_w, y), base.with("cancel"));
                ok.show(ui, ok_min, base.with("ok"));
            }
        });
    if !closing && ctx.input(|i| i.key_pressed(Key::Escape)) {
        outcome = Outcome::Cancel;
    }
    outcome
}

fn focus_once(ctx: &Context, resp: &eframe::egui::Response, serial: u64) {
    let id = Id::new("oc-dialog-focused");
    if ctx.data(|d| d.get_temp::<u64>(id)) != Some(serial) {
        resp.request_focus();
        ctx.data_mut(|d| d.insert_temp(id, serial));
    }
}

pub fn progress(ctx: &Context, serial: u64, open: bool) -> f32 {
    motion::tween_from(ctx, Id::new(("oc-dialog-k", serial)), 0.0, if open { 1.0 } else { 0.0 }, 0.16, motion::EASE)
}
