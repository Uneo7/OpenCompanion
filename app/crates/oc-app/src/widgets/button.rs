use eframe::egui::{self, Align, Color32, CursorIcon, Id, Pos2, Rect, Response, Sense, Vec2};

use super::{paint, text, text::st, tooltip};
use crate::assets::icons::Icon;
use crate::theme::{self, motion, typography::W, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Default,
    Primary,
    Quiet,
    Danger,
}

#[derive(Clone)]
pub struct Button<'a> {
    pub label: &'a str,
    pub icon: Option<Icon>,
    pub kind: Kind,
    pub small: bool,
    pub height: Option<f32>,
    pub enabled: bool,
}

impl<'a> Button<'a> {
    pub fn new(label: &'a str) -> Self {
        Self { label, icon: None, kind: Kind::Default, small: false, height: None, enabled: true }
    }

    pub fn icon(mut self, i: Icon) -> Self {
        self.icon = Some(i);
        self
    }

    pub fn kind(mut self, k: Kind) -> Self {
        self.kind = k;
        self
    }

    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    fn style(&self) -> text::Style {
        if self.small {
            st::sec(W::Medium)
        } else {
            st::body(W::Medium)
        }
    }

    fn pad(&self) -> f32 {
        if self.small {
            12.0
        } else {
            16.0
        }
    }

    pub fn size(&self, ctx: &egui::Context) -> Vec2 {
        let icon = if self.icon.is_some() { 16.0 + 8.0 } else { 0.0 };
        let w = self.pad() * 2.0 + icon + text::width(ctx, self.label, &self.style());
        Vec2::new(w.round(), self.height.unwrap_or(if self.small { 30.0 } else { 36.0 }))
    }

    pub fn show(self, ui: &mut egui::Ui, min: Pos2, id: Id) -> Response {
        let rect = Rect::from_min_size(min, self.size(ui.ctx()));
        self.show_in(ui, rect, id)
    }

    fn colors(&self, t: &Theme, h: f32, a: f32) -> (Color32, Color32) {
        match self.kind {
            Kind::Default => (theme::mix(theme::mix(t.field, t.field_hover, h), t.field_active, a), t.text),
            Kind::Primary => (theme::mix(theme::mix(t.fill, t.fill_hover, h), t.fill_active, a), t.on_fill),
            Kind::Quiet => (theme::alpha(t.field, h.max(a)), theme::mix(t.muted, t.text, h)),
            Kind::Danger => (theme::mix(t.danger_fill, t.danger_hover, h), t.on_danger),
        }
    }

    pub fn show_in(self, ui: &mut egui::Ui, rect: Rect, id: Id) -> Response {
        let ctx = ui.ctx().clone();
        let t = theme::get(&ctx);
        let sense = if self.enabled { Sense::click() } else { Sense::hover() };
        let resp = ui.interact(rect, id, sense);
        let down = self.enabled && resp.is_pointer_button_down_on();
        let h = motion::hover(&ctx, id, self.enabled && resp.hovered());
        let a = motion::toggle(&ctx, id.with("active"), down, 0.12, motion::EASE);
        let s = 1.0 - 0.03 * a;
        if self.enabled && resp.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        let painter = paint::faded(ui.painter(), if self.enabled { 1.0 } else { t.disabled });
        let r = Rect::from_center_size(rect.center(), rect.size() * s);
        let (bg, fg) = self.colors(&t, h, a);
        painter.rect_filled(r, 8.0 * s, bg);
        let style = self.style();
        let style = text::Style { font: egui::FontId::new(style.font.size * s, style.font.family.clone()), ..style };
        let content = self.size(&ctx).x - 2.0 * self.pad();
        let mut x = r.center().x - content * s / 2.0;
        if let Some(i) = self.icon {
            i.paint(&painter, Pos2::new(x + 8.0 * s, r.center().y), 16.0 * s, fg);
            x += 24.0 * s;
        }
        text::line(&painter, self.label, &style, fg, x, r.top(), r.height(), Align::Min);
        resp
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconKind {
    #[default]
    Ghost,
    Round,
}

#[derive(Clone, Copy)]
pub struct IconButton<'a> {
    pub icon: Icon,
    pub kind: IconKind,
    pub on: bool,
    /// `aria-expanded`: keeps the hover look while its menu is open.
    pub open: bool,
    pub danger: bool,
    pub enabled: bool,
    pub tip: Option<&'a str>,
    pub kbd: Option<&'a str>,
}

impl<'a> IconButton<'a> {
    pub fn new(icon: Icon) -> Self {
        Self { icon, kind: IconKind::Ghost, on: false, open: false, danger: false, enabled: true, tip: None, kbd: None }
    }

    pub fn round(mut self) -> Self {
        self.kind = IconKind::Round;
        self
    }

    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }

    pub fn tip(mut self, tip: &'a str, kbd: Option<&'a str>) -> Self {
        self.tip = Some(tip);
        self.kbd = kbd;
        self
    }

    pub fn show(self, ui: &mut egui::Ui, rect: Rect, id: Id) -> Response {
        let ctx = ui.ctx().clone();
        let t = theme::get(&ctx);
        let sense = if self.enabled { Sense::click() } else { Sense::hover() };
        let resp = ui.interact(rect, id, sense);
        if self.enabled && resp.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        if let (Some(tip), true) = (self.tip, !self.open) {
            tooltip::attach(&resp, tip, self.kbd);
        }
        let down = self.enabled && resp.is_pointer_button_down_on();
        let h = motion::hover(&ctx, id, self.enabled && (resp.hovered() || self.open));
        let a = motion::toggle(&ctx, id.with("active"), down, 0.12, motion::EASE);
        let k = motion::toggle(&ctx, id.with("on"), self.on, 0.15, motion::EASE);
        let s = 1.0 - 0.06 * a;
        let r = Rect::from_center_size(rect.center(), rect.size() * s);
        let radius = if self.kind == IconKind::Round { r.width() / 2.0 } else { 8.0 * s };
        let painter = paint::faded(ui.painter(), if self.enabled { 1.0 } else { t.disabled });
        let (rest_bg, rest_fg) = match self.kind {
            IconKind::Ghost => (theme::mix(theme::alpha(t.field, h), t.field_active, a), theme::mix(t.muted, t.text, h)),
            IconKind::Round => (theme::mix(theme::mix(t.field, t.field_hover, h), t.field_active, a), t.text),
        };
        let on_bg = theme::mix(t.fill, t.fill_hover, h);
        let bg = theme::mix(rest_bg, on_bg, k);
        let mut fg = theme::mix(rest_fg, t.on_fill, k);
        if self.danger {
            fg = theme::mix(t.muted, t.crit, h);
        }
        painter.rect_filled(r, radius, bg);
        let icon = if rect.width() <= 30.5 { 16.0 } else { 18.0 };
        self.icon.paint(&painter, r.center(), icon * s, fg);
        resp
    }
}
