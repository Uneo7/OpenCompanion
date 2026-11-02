use eframe::egui::{self, Align, CursorIcon, Id, Pos2, Rect, Response, Sense, Vec2};

use super::{paint, text, text::st, tooltip};
use crate::assets::icons::Icon;
use crate::theme::{self, motion, typography::W};

#[derive(Clone, Copy)]
pub struct Segment<'a> {
    pub icon: Option<Icon>,
    pub label: &'a str,
    pub tip: Option<&'a str>,
}

impl<'a> Segment<'a> {
    pub fn text(label: &'a str) -> Self {
        Self { icon: None, label, tip: None }
    }

    pub fn icon(icon: Icon, label: &'a str) -> Self {
        Self { icon: Some(icon), label, tip: None }
    }

    pub fn tip(mut self, tip: &'a str) -> Self {
        self.tip = Some(tip);
        self
    }
}

const PAD: f32 = 12.0;
const ICON: f32 = 16.0;
const ICON_GAP: f32 = 6.0;
const H: f32 = 30.0;

fn seg_width(ctx: &egui::Context, s: &Segment) -> f32 {
    let iw = if s.icon.is_some() { ICON + ICON_GAP } else { 0.0 };
    (2.0 * PAD + iw + text::width(ctx, s.label, &st::sec(W::Medium))).round()
}

/// Total size of the control (the `_seg_ic` flag is kept for callers: both variants share one size now).
pub fn size(ctx: &egui::Context, segs: &[Segment], _seg_ic: bool) -> Vec2 {
    let w: f32 = segs.iter().map(|s| seg_width(ctx, s)).sum::<f32>() + 2.0 * (segs.len() as f32 - 1.0) + 6.0;
    Vec2::new(w, H + 6.0)
}

pub fn segmented(ui: &mut egui::Ui, min: Pos2, id: Id, segs: &[Segment], selected: Option<usize>, seg_ic: bool) -> (Option<usize>, Rect) {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let rect = Rect::from_min_size(min, size(&ctx, segs, seg_ic));
    ui.painter().rect_filled(rect, 10.0, t.field);

    let mut boxes = Vec::with_capacity(segs.len());
    let mut x = rect.left() + 3.0;
    for s in segs {
        let w = seg_width(&ctx, s);
        boxes.push(Rect::from_min_size(Pos2::new(x, rect.top() + 3.0), Vec2::new(w, H)));
        x += w + 2.0;
    }

    // Sliding chip: tweened left edge and width (instant the first time it is drawn).
    if let Some(sel) = selected.and_then(|i| boxes.get(i)) {
        let l = motion::tween(&ctx, id.with("ind-l"), sel.left() - rect.left(), 0.26, motion::SPRING);
        let w = motion::tween(&ctx, id.with("ind-w"), sel.width(), 0.26, motion::SPRING);
        let chip = Rect::from_min_size(Pos2::new(rect.left() + l, sel.top()), Vec2::new(w, H));
        paint::shadow(ui.painter(), chip, 7.0, 1.0, 2.0, 0.0, t.seg_on_shadow);
        ui.painter().rect_filled(chip, 7.0, t.field_on);
    }

    let mut clicked = None;
    for (i, (s, r)) in segs.iter().zip(&boxes).enumerate() {
        let sid = id.with(i);
        let resp: Response = ui.interact(*r, sid, Sense::click());
        if resp.clicked() {
            clicked = Some(i);
        }
        if resp.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        if let Some(tip) = s.tip {
            tooltip::attach(&resp, tip, None);
        }
        let on = selected == Some(i);
        let k = motion::toggle(&ctx, sid.with("on"), on, 0.15, motion::EASE);
        let h = motion::hover(&ctx, sid, resp.hovered());
        let fg = theme::mix(theme::mix(t.muted, t.text, h), t.text, k);
        let mut tx = r.left() + PAD;
        if let Some(ic) = s.icon {
            ic.paint(ui.painter(), Pos2::new(tx + ICON / 2.0, r.center().y), ICON, theme::mix(fg, t.a1, k));
            tx += ICON + ICON_GAP;
        }
        text::line(ui.painter(), s.label, &st::sec(W::Medium), fg, tx, r.top(), H, Align::Min);
    }
    (clicked, rect)
}
