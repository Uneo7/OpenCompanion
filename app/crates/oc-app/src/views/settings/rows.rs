use eframe::egui::{self, Align, Color32, CursorIcon, Id, Pos2, Rect, Response, Sense, Stroke, Vec2};

use crate::assets::icons::Icon;
use crate::theme::{self, motion, typography::W};

use crate::widgets::{menu, slider, text, text::st, tooltip};

pub struct Page {
    pub left: f32,
    pub right: f32,
    pub y: f32,
    after_head: bool,
    started: bool,
}

impl Page {
    pub fn new(r: Rect) -> Self {
        Self { left: r.left(), right: r.right(), y: r.top(), after_head: false, started: false }
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    fn gap(&mut self) {
        if self.started {
            self.y += 16.0;
        }
        self.started = true;
    }

    pub fn head(&mut self, ui: &egui::Ui, title: &str, sec: Option<(&str, Option<&str>)>) {
        let t = theme::get(ui.ctx());
        self.gap();
        let top = self.y + 4.0;
        let style = st::pane();
        let r = text::line(ui.painter(), title, &style, t.text, self.left, top + 3.0, 26.0, Align::Min);
        if let Some((s, strong)) = sec {
            let base = top + 3.0 + text::baseline_in_box(ui.ctx(), &style, 26.0);
            let secs = st::sec(W::Regular);
            let lt = base - text::baseline_in_box(ui.ctx(), &secs, 18.0);
            let r = text::line(ui.painter(), s, &secs, t.muted, r.right() + 12.0, lt, 18.0, Align::Min);
            if let Some(b) = strong {
                text::line(ui.painter(), b, &st::sec(W::Medium), t.text, r.right() + 4.0, lt, 18.0, Align::Min);
            }
        }
        self.y = top + 32.0 + 8.0;
        self.after_head = true;
    }

    /// The head with a status on the title's line, vertically centred on it; `busy`: a spinner before it.
    pub fn head_status(&mut self, ui: &egui::Ui, title: &str, status: &str, busy: bool) {
        let t = theme::get(ui.ctx());
        self.gap();
        let top = self.y + 4.0;
        let r = text::line(ui.painter(), title, &st::pane(), t.text, self.left, top + 3.0, 26.0, Align::Min);
        let cy = top + 3.0 + 13.0;
        let mut x = r.right() + 14.0;
        if busy {
            crate::widgets::spinner::spinner(ui, Pos2::new(x + 7.0, cy), 7.0);
            x += 14.0 + 8.0;
        }
        text::line(ui.painter(), status, &st::sec(W::Regular), t.muted, x, cy - 9.0, 18.0, Align::Min);
        self.y = top + 32.0 + 8.0;
        self.after_head = true;
    }

    /// `.sec-t` (12 px medium, muted): 10 px above (none right after the head), pulls the next block 8 px up.
    pub fn section(&mut self, ui: &egui::Ui, title: &str) {
        let t = theme::get(ui.ctx());
        self.gap();
        if !self.after_head {
            self.y += 10.0;
        }
        self.after_head = false;
        text::line(ui.painter(), title, &st::sec_t(), t.muted, self.left, self.y, 16.0, Align::Min);
        self.y += 16.0 - 8.0;
    }

    pub fn block(&mut self, h: f32) -> Rect {
        self.gap();
        self.after_head = false;
        let r = Rect::from_min_size(Pos2::new(self.left, self.y), Vec2::new(self.width(), h));
        self.y += h;
        r
    }

    pub fn list(&mut self, ui: &egui::Ui, heights: &[f32]) -> Vec<Rect> {
        let t = theme::get(ui.ctx());
        let total = 2.0 + heights.iter().sum::<f32>();
        let r = self.block(total);
        let line = |y: f32| ui.painter().line_segment([Pos2::new(r.left(), y + 0.5), Pos2::new(r.right(), y + 0.5)], Stroke::new(1.0_f32, t.sep));
        line(r.top());
        line(r.bottom() - 1.0);
        let mut y = r.top() + 1.0;
        let mut rows = Vec::new();
        for (i, h) in heights.iter().enumerate() {
            if i > 0 {
                line(y - 0.5);
            }
            rows.push(Rect::from_min_size(Pos2::new(r.left(), y), Vec2::new(r.width(), *h)));
            y += h;
        }
        rows
    }
}

pub fn row_text(ui: &egui::Ui, x: f32, cy: f32, title: &str, sub: Option<&str>, title_color: Option<Color32>) {
    let t = theme::get(ui.ctx());
    let tc = title_color.unwrap_or(t.text);
    match sub {
        Some(s) => {
            let top = cy - 19.0;
            text::line(ui.painter(), title, &st::body(W::Medium), tc, x, top, 20.0, Align::Min);
            text::line(ui.painter(), s, &st::sec(W::Regular), t.muted, x, top + 20.0, 18.0, Align::Min);
        }
        None => {
            text::line(ui.painter(), title, &st::body(W::Medium), tc, x, cy - 10.0, 20.0, Align::Min);
        }
    }
}

pub fn row_icon(ui: &egui::Ui, row: Rect, icon: Icon) -> f32 {
    let t = theme::get(ui.ctx());
    icon.paint(ui.painter(), Pos2::new(row.left() + 9.0, row.center().y), 18.0, t.muted);
    row.left() + 18.0 + 16.0
}

#[allow(clippy::too_many_arguments)]
pub fn slider_out(ui: &mut egui::Ui, right: f32, cy: f32, id: Id, value: &mut f32, range: (f32, f32, f32), default: f32, out: &str) -> bool {
    let left = right - 240.0;
    let r = Rect::from_min_size(Pos2::new(left, cy - 12.0), Vec2::new(132.0, 24.0));
    let o = slider::Opts { reset: Some(default), ..Default::default() };
    let changed = slider::slider_ex(ui, r, id, value, range, slider::Look::Settings, o).changed();
    output(ui, right, cy, out);
    changed
}

#[allow(clippy::too_many_arguments)]
pub fn slider_off(ui: &mut egui::Ui, right: f32, cy: f32, id: Id, value: f32, range: (f32, f32, f32), out: &str, tip: &str) {
    let r = Rect::from_min_size(Pos2::new(right - 240.0, cy - 12.0), Vec2::new(132.0, 24.0));
    let mut v = value;
    let o = slider::Opts { disabled: true, ..Default::default() };
    let resp = slider::slider_ex(ui, r, id, &mut v, range, slider::Look::Settings, o);
    crate::widgets::tooltip::attach(&resp, tip, None);
    output(ui, right, cy, out);
}

pub fn slider_ro(ui: &mut egui::Ui, right: f32, cy: f32, id: Id, value: f32, range: (f32, f32, f32), out: &str) {
    let r = Rect::from_min_size(Pos2::new(right - 240.0, cy - 12.0), Vec2::new(132.0, 24.0));
    let mut v = value;
    let o = slider::Opts { read_only: true, ..Default::default() };
    slider::slider_ex(ui, r, id, &mut v, range, slider::Look::Settings, o);
    output(ui, right, cy, out);
}

pub fn output(ui: &egui::Ui, right: f32, cy: f32, out: &str) {
    let t = theme::get(ui.ctx());
    text::line(ui.painter(), out, &st::mono_(13.0, W::Regular), t.muted, right, cy - 9.0, 18.0, Align::Max);
}

pub fn value_text_rect(ui: &egui::Ui, right: f32, cy: f32, s: &str) -> Rect {
    let t = theme::get(ui.ctx());
    text::line(ui.painter(), s, &st::sec(W::Regular), t.muted, right, cy - 9.0, 18.0, Align::Max)
}

pub fn value_text(ui: &egui::Ui, right: f32, cy: f32, s: &str, dot: bool) {
    let t = theme::get(ui.ctx());
    let r = text::line(ui.painter(), s, &st::sec(W::Regular), t.muted, right, cy - 9.0, 18.0, Align::Max);
    if dot {
        ui.painter().circle_filled(Pos2::new(r.left() - 8.0 - 3.5, cy), 3.5, t.ok);
    }
}

pub fn select(ui: &mut egui::Ui, right: f32, cy: f32, id: Id, label: &str, open: bool, tip: Option<&str>) -> Response {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let style = st::body(W::Regular);
    let w = (12.0 + text::width(&ctx, label, &style) + 8.0 + 14.0 + 8.0).max(150.0);
    let r = Rect::from_min_size(Pos2::new(right - w, cy - 18.0), Vec2::new(w, 36.0));
    let resp = ui.interact(r, id, Sense::click());
    if resp.hovered() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
    if let Some(tip) = tip {
        tooltip::attach(&resp, tip, None);
    }
    let h = motion::hover(&ctx, id, resp.hovered() || open);
    ui.painter().rect_filled(r, 8.0, theme::mix(t.field, t.field_hover, h));
    text::line(ui.painter(), label, &style, t.text, r.left() + 12.0, r.top(), 36.0, Align::Min);
    menu::chevron(ui, Pos2::new(r.right() - 8.0 - 7.0, r.center().y), id.with("chev"), open);
    resp
}
