use eframe::egui::{
    Align, Area, Color32, Context, CursorIcon, Id, Key, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};

use super::{paint, switch, text, text::st};
use crate::assets::icons::{self, Icon};
use crate::theme::{self, motion, typography::W};

#[derive(Clone)]
pub enum Lead {
    None,
    Icon(Icon),
    Mark(Color32),
}

#[derive(Clone)]
pub enum Meta {
    None,
    Toggle(bool),
    Check,
    Kbd(String),
}

#[derive(Clone)]
pub enum Entry<A> {
    Group(String),
    Sep,
    Item { label: String, lead: Lead, meta: Meta, danger: bool, action: A },
    Pairs(Vec<(String, String)>),
}

impl<A> Entry<A> {
    pub fn item(label: impl Into<String>, icon: Icon, action: A) -> Self {
        Entry::Item { label: label.into(), lead: Lead::Icon(icon), meta: Meta::None, danger: false, action }
    }

    pub fn plain(label: impl Into<String>, action: A) -> Self {
        Entry::Item { label: label.into(), lead: Lead::None, meta: Meta::None, danger: false, action }
    }

    pub fn lead(mut self, l: Lead) -> Self {
        if let Entry::Item { lead, .. } = &mut self {
            *lead = l;
        }
        self
    }

    pub fn meta(mut self, m: Meta) -> Self {
        if let Entry::Item { meta, .. } = &mut self {
            *meta = m;
        }
        self
    }

    pub fn danger(mut self) -> Self {
        if let Entry::Item { danger, .. } = &mut self {
            *danger = true;
        }
        self
    }

    fn height(&self) -> f32 {
        match self {
            Entry::Group(_) => 26.0,
            Entry::Sep => 9.0,
            Entry::Item { .. } => 34.0,
            Entry::Pairs(rows) => rows.len() as f32 * 16.0 + rows.len().saturating_sub(1) as f32 * 10.0 + 10.0,
        }
    }
}

#[derive(Clone, Copy)]
pub enum Anchor {
    /// Under (or above) a control; the control toggles the menu.
    Rect(Rect),
    Point(Pos2),
}

#[derive(Clone)]
pub struct Menu<A> {
    /// Identifies the opener (same key = same menu, so the opener toggles it).
    pub key: Id,
    pub anchor: Anchor,
    pub align_right: bool,
    pub entries: Vec<Entry<A>>,
    pub serial: u64,
}

pub enum Outcome<A> {
    Stay,
    Close,
    Pick(A),
}

const MIN_W: f32 = 248.0;
const MARGIN: f32 = 8.0;
const PAIR_GAP: f32 = 24.0;

fn pair_style(bold: bool) -> text::Style {
    st::inter_(13.0, if bold { W::Medium } else { W::Regular })
}

fn pair_cols(ctx: &Context, rows: &[(String, String)]) -> (f32, f32) {
    let a = rows.iter().map(|(d, _)| text::width(ctx, d, &pair_style(true))).fold(0.0, f32::max);
    let b = rows.iter().map(|(_, e)| text::width(ctx, e, &pair_style(false))).fold(0.0, f32::max);
    (a, b)
}

fn width<A>(ctx: &Context, entries: &[Entry<A>]) -> f32 {
    let label = st::sec(W::Medium);
    let content = entries
        .iter()
        .map(|e| match e {
            Entry::Item { label: l, meta, lead, .. } => {
                let meta_w = match meta {
                    Meta::None => 0.0,
                    Meta::Toggle(_) => 28.0,
                    Meta::Check => 16.0,
                    Meta::Kbd(k) => 10.0 + text::width(ctx, k, &st::mono_(11.0, W::Medium)),
                };
                let lead_w = if matches!(lead, Lead::None) { 0.0 } else { 26.0 };
                10.0 + lead_w + text::width(ctx, l, &label) + 10.0 + meta_w + 10.0
            }
            Entry::Group(g) => 20.0 + text::width(ctx, g, &group_style()),
            Entry::Sep => 0.0,
            Entry::Pairs(rows) => {
                let (a, b) = pair_cols(ctx, rows);
                20.0 + a + PAIR_GAP + b
            }
        })
        .fold(0.0, f32::max);
    (content + 2.0 * 4.0 + 2.0).max(MIN_W).ceil()
}

fn group_style() -> text::Style {
    st::mono_(10.0, W::Medium).tracking(0.06).upper()
}

/// Placement inside `window` (8 px margin): below the anchor if it fits, else above, else the side with more
/// room; horizontally the requested alignment, flipped when it would overflow. Returns (rect, opens upwards).
fn place(anchor: Anchor, align_right: bool, size: Vec2, window: Rect) -> (Rect, bool) {
    let (p, gap) = match anchor {
        Anchor::Rect(r) => (r, 6.0),
        Anchor::Point(p) => (Rect::from_min_max(p, p), 2.0),
    };
    let (w, h) = (size.x, size.y);
    let mut x = if align_right { p.right() - w } else { p.left() };
    if x + w > window.right() - MARGIN {
        x = p.right() - w;
    }
    if x < window.left() + MARGIN {
        x = p.left();
    }
    x = x.clamp(window.left() + MARGIN, window.right() - w - MARGIN);
    let below = window.bottom() - MARGIN - (p.bottom() + gap);
    let above = p.top() - gap - window.top() - MARGIN;
    let y = if h <= below {
        p.bottom() + gap
    } else if h <= above {
        p.top() - gap - h
    } else if below >= above {
        window.bottom() - MARGIN - h
    } else {
        window.top() + MARGIN
    };
    let y = y.clamp(window.top() + MARGIN, (window.bottom() - h - MARGIN).max(window.top() + MARGIN));
    (Rect::from_min_size(Pos2::new(x, y), size), y < p.top())
}

pub fn show<A: Clone>(ctx: &Context, menu: &Menu<A>, window: Rect) -> Outcome<A> {
    let t = theme::get(ctx);
    let size = Vec2::new(width(ctx, &menu.entries), 2.0 + 8.0 + menu.entries.iter().map(Entry::height).sum::<f32>());
    let (rect, upwards) = place(menu.anchor, menu.align_right, size, window);
    let k = motion::EASE_OUT.at(motion::since(ctx, menu.key.with("pop"), menu.serial, 0.12));
    let shift = Vec2::new(0.0, if upwards { 4.0 } else { -4.0 } * (1.0 - k));

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        return Outcome::Close;
    }
    let mut outcome = Outcome::Stay;
    let items: Vec<usize> = menu.entries.iter().enumerate().filter(|(_, e)| matches!(e, Entry::Item { .. })).map(|(i, _)| i).collect();
    let focus_id = menu.key.with(("focus", menu.serial));
    let mut focus: Option<usize> = ctx.data(|d| d.get_temp(focus_id)).unwrap_or(None);
    if !items.is_empty() {
        let (down, up, enter) = ctx.input(|i| (i.key_pressed(Key::ArrowDown), i.key_pressed(Key::ArrowUp), i.key_pressed(Key::Enter)));
        if down {
            focus = Some(focus.map_or(0, |f| (f + 1) % items.len()));
        }
        if up {
            focus = Some(focus.map_or(items.len() - 1, |f| (f + items.len() - 1) % items.len()));
        }
        ctx.data_mut(|d| d.insert_temp(focus_id, focus));
        if let (true, Some(f)) = (enter, focus) {
            if let Some(Entry::Item { action, .. }) = menu.entries.get(items[f]) {
                return Outcome::Pick(action.clone());
            }
        }
    }
    let focused = focus.and_then(|f| items.get(f).copied());
    if let Some(press) = ctx.input(|i| if i.pointer.any_pressed() { i.pointer.press_origin() } else { None }) {
        let on_anchor = matches!(menu.anchor, Anchor::Rect(r) if r.contains(press));
        if !rect.contains(press) && !on_anchor {
            outcome = Outcome::Close;
        }
    }

    Area::new(menu.key.with("menu-area"))
        .order(Order::Foreground)
        .fixed_pos(rect.min)
        .constrain(false)
        .show(ctx, |ui| {
            let _ = ui.allocate_exact_size(size, Sense::click());
            let mut painter = ui.painter().clone();
            painter.set_opacity(k);
            let r = rect.translate(shift);
            paint::shadow(&painter, r, 12.0, 12.0, 32.0, 0.0, t.pop_shadow);
            painter.rect(r, 12.0, t.pop, Stroke::new(1.0_f32, t.pop_edge), StrokeKind::Inside);
            let mut y = r.top() + 1.0 + 4.0;
            let (left, right) = (r.left() + 1.0 + 4.0, r.right() - 1.0 - 4.0);
            for (i, e) in menu.entries.iter().enumerate() {
                let h = e.height();
                let row = Rect::from_min_max(Pos2::new(left, y), Pos2::new(right, y + h));
                match e {
                    Entry::Group(g) => {
                        text::line(&painter, g, &group_style(), t.muted, row.left() + 10.0, row.top() + 6.0, 16.0, Align::Min);
                    }
                    Entry::Sep => {
                        let yy = row.center().y.round() - 0.5;
                        painter.line_segment([Pos2::new(row.left() + 4.0, yy), Pos2::new(row.right() - 4.0, yy)], Stroke::new(1.0_f32, t.sep));
                    }
                    Entry::Pairs(rows) => {
                        let (a, _) = pair_cols(ui.ctx(), rows);
                        let mut yy = row.top() + 2.0;
                        for (d, e) in rows {
                            text::line(&painter, d, &pair_style(true), t.text, row.left() + 10.0, yy, 16.0, Align::Min);
                            text::line(&painter, e, &pair_style(false), t.muted, row.left() + 10.0 + a + PAIR_GAP, yy, 16.0, Align::Min);
                            yy += 26.0;
                        }
                    }
                    Entry::Item { label, lead, meta, danger, action } => {
                        let id = menu.key.with(("item", i));
                        let resp = ui.interact(row.translate(-shift), id, Sense::click());
                        let hovered = resp.hovered();
                        let hk = motion::toggle(ui.ctx(), id.with("h"), hovered || focused == Some(i), 0.1, motion::EASE);
                        if hk > 0.0 {
                            painter.rect_filled(row, 6.0, theme::alpha(t.field, hk));
                        }
                        if hovered {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                        let fg = if *danger { t.crit } else { t.text };
                        let icon_c = if *danger { t.crit } else { theme::mix(t.muted, t.text, hk) };
                        let c = Pos2::new(row.left() + 10.0 + 8.0, row.center().y);
                        let lx = match lead {
                            Lead::None => 10.0,
                            Lead::Icon(ic) => {
                                ic.paint(&painter, c, 16.0, icon_c);
                                36.0
                            }
                            Lead::Mark(col) => {
                                icons::DOT.paint(&painter, c, 10.0, *col);
                                36.0
                            }
                        };
                        let checked = matches!(meta, Meta::Check);
                        let style = st::sec(if checked { W::Medium } else { W::Regular });
                        text::line(&painter, label, &style, fg, row.left() + lx, row.top(), h, Align::Min);
                        let mr = row.right() - 10.0;
                        match meta {
                            Meta::None => {}
                            Meta::Toggle(on) => {
                                let tr = Rect::from_min_size(Pos2::new(mr - 28.0, row.center().y - 8.0), Vec2::new(28.0, 16.0));
                                switch::paint_toggle(&painter, ui.ctx(), tr, id.with("tg"), *on, false, &t, 2.0);
                            }
                            Meta::Check => icons::CHECK.paint(&painter, Pos2::new(mr - 8.0, row.center().y), 16.0, t.a1),
                            Meta::Kbd(k) => {
                                let style = st::mono_(11.0, W::Medium);
                                let kw = text::width(ui.ctx(), k, &style) + 10.0;
                                let kr = Rect::from_min_size(Pos2::new(mr - kw, row.center().y - 9.0), Vec2::new(kw, 18.0));
                                painter.rect_filled(kr, 4.0, t.field);
                                text::line(&painter, k, &style, t.muted, kr.center().x, kr.top(), 18.0, Align::Center);
                            }
                        }
                        if resp.clicked() {
                            outcome = Outcome::Pick(action.clone());
                        }
                    }
                }
                y += h;
            }
        });
    outcome
}

pub fn chevron(ui: &eframe::egui::Ui, center: Pos2, id: Id, open: bool) {
    let t = theme::get(ui.ctx());
    let k = motion::toggle(ui.ctx(), id, open, 0.15, motion::EASE);
    icons::CHEVRON_DOWN.paint_rotated(ui.painter(), center, 14.0, t.muted, std::f32::consts::PI * k);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_flips_and_clamps() {
        let win = Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 720.0));
        let size = Vec2::new(248.0, 300.0);
        // Room below: under the anchor, left-aligned.
        let (r, up) = place(Anchor::Rect(Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(80.0, 30.0))), false, size, win);
        assert_eq!((r.left(), r.top(), up), (100.0, 136.0, false));
        // Near the bottom: above.
        let (r, up) = place(Anchor::Rect(Rect::from_min_size(Pos2::new(100.0, 600.0), Vec2::new(80.0, 30.0))), false, size, win);
        assert!(up && (r.bottom() - 594.0).abs() < 1e-3);
        // Right-aligned near the right edge stays inside.
        let (r, _) = place(Anchor::Rect(Rect::from_min_size(Pos2::new(1060.0, 50.0), Vec2::new(36.0, 36.0))), true, size, win);
        assert_eq!(r.right(), 1092.0);
        // Point near the right edge flips to the left of the pointer.
        let (r, _) = place(Anchor::Point(Pos2::new(1000.0, 200.0)), false, size, win);
        assert_eq!(r.right(), 1000.0);
    }

    #[test]
    fn pair_rows_have_height() {
        let e: Entry<()> = Entry::Pairs(vec![("a".into(), "b".into()), ("c".into(), "d".into())]);
        assert_eq!(e.height(), 16.0 * 2.0 + 10.0 + 10.0);
    }
}
