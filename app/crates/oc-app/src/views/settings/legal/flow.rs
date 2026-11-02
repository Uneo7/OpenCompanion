use std::sync::Arc;

use eframe::egui::{self, Align, Id, Pos2, Stroke};

use super::docs::Doc;
use crate::i18n::{t, tr};
use crate::theme::{typography::W, Theme};
use crate::widgets::{button, text, text::st};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    Doc(Doc),
    /// A web page (only where no text is embedded).
    Web(&'static str),
}

/// One row of a list: title, copyright line, licence, link; `pending` replaces the button label and disables it.
pub struct Item {
    pub title: String,
    pub sub: String,
    pub value: String,
    pub link: Option<Link>,
    pub pending: Option<&'static str>,
}

pub struct Flow<'a> {
    pub ui: &'a mut egui::Ui,
    pub t: Arc<Theme>,
    pub left: f32,
    pub width: f32,
    pub y: f32,
    pub first: bool,
    pub clicked: Option<Link>,
}

impl Flow<'_> {
    fn gap(&mut self) {
        if !self.first {
            self.y += 8.0;
        }
        self.first = false;
    }

    fn hairline(&self, y: f32) {
        let stroke = Stroke::new(1.0_f32, self.t.sep);
        self.ui.painter().line_segment([Pos2::new(self.left, y + 0.5), Pos2::new(self.left + self.width, y + 0.5)], stroke);
    }

    pub fn sec(&mut self, key: &'static str) {
        let first = self.first;
        self.gap();
        if !first {
            self.y += 16.0;
        }
        text::line(self.ui.painter(), tr(key), &st::sec_t(), self.t.muted, self.left, self.y, 16.0, Align::Min);
        self.y += 16.0;
    }

    pub fn sub(&mut self, key: &'static str) {
        self.gap();
        self.y += 8.0;
        text::line(self.ui.painter(), tr(key), &st::inter_(12.0, W::Regular), self.t.muted, self.left, self.y, 16.0, Align::Min);
        self.y += 16.0;
    }

    pub fn p(&mut self, key: &'static str) {
        self.gap();
        let h = text::paragraph(self.ui.painter(), tr(key), &st::sec(W::Regular), self.t.muted, Pos2::new(self.left, self.y), self.width, 20.0);
        self.y += h;
    }

    pub fn bullets(&mut self) {
        self.gap();
        let bold = st::sec(W::Medium);
        let body = st::sec(W::Regular);
        for i in 1..=5 {
            if i > 1 {
                self.y += 6.0;
            }
            let lead = t(&format!("legal.risk.{i}.lead"));
            let rest = format!(" {}", t(&format!("legal.risk.{i}")));
            text::line(self.ui.painter(), "•", &body, self.t.muted, self.left + 6.0, self.y, 20.0, Align::Min);
            let runs = [(lead.as_str(), &bold, self.t.text), (rest.as_str(), &body, self.t.muted)];
            self.y += text::rich_paragraph(self.ui.painter(), &runs, Pos2::new(self.left + 18.0, self.y), self.width - 18.0, 20.0);
        }
    }

    fn row_text(&mut self, title: &str, sub: &str, text_w: f32) -> f32 {
        let ctx = self.ui.ctx().clone();
        let vs = st::inter_(12.0, W::Regular);
        let sub_h = text::paragraph_height(&ctx, sub, &vs, text_w, 16.0);
        let h = (8.0 + 18.0 + sub_h + 8.0).max(52.0);
        let top = self.y + h / 2.0 - (18.0 + sub_h) / 2.0;
        text::line(self.ui.painter(), title, &st::sec(W::Medium), self.t.text, self.left, top, 18.0, Align::Min);
        text::paragraph(self.ui.painter(), sub, &vs, self.t.muted, Pos2::new(self.left, top + 18.0), text_w, 16.0);
        h
    }

    pub fn list(&mut self, items: &[Item], id: &str) {
        self.gap();
        let ctx = self.ui.ctx().clone();
        self.hairline(self.y);
        self.y += 1.0;
        for (k, it) in items.iter().enumerate() {
            if k > 0 {
                self.hairline(self.y - 0.5);
            }
            let label = it.pending.map_or(tr("legal.view"), tr);
            let btn = button::Button::new(label).small().kind(button::Kind::Quiet).enabled(it.pending.is_none() && it.link.is_some());
            let bw = btn.size(&ctx).x;
            let vs = st::inter_(12.0, W::Regular);
            let vw = if it.value.is_empty() { 0.0 } else { text::width(&ctx, &it.value, &vs) + 12.0 };
            let h = self.row_text(&it.title, &it.sub, self.width - bw - 12.0 - vw);
            let cy = self.y + h / 2.0;
            let bx = self.left + self.width - bw;
            if !it.value.is_empty() {
                text::line(self.ui.painter(), &it.value, &vs, self.t.muted, bx - 12.0, cy - 9.0, 18.0, Align::Max);
            }
            if btn.show(self.ui, Pos2::new(bx, cy - 15.0), Id::new((id, k))).clicked() {
                self.clicked = it.link;
            }
            self.y += h;
        }
        self.hairline(self.y);
        self.y += 1.0;
    }

    pub fn licence_row(&mut self, title: &str, sub: &str) {
        self.gap();
        let ctx = self.ui.ctx().clone();
        let gpl = button::Button::new(tr("legal.lic.gpl")).small().kind(button::Kind::Quiet);
        let gw = gpl.size(&ctx).x;
        self.hairline(self.y);
        self.y += 1.0;
        let h = self.row_text(title, sub, self.width - gw - 12.0);
        let cy = self.y + h / 2.0;
        let gx = self.left + self.width - gw;
        if gpl.show(self.ui, Pos2::new(gx, cy - 15.0), Id::new("lic-gpl")).clicked() {
            self.clicked = Some(Link::Doc(Doc::Gpl));
        }
        self.y += h;
        self.hairline(self.y);
        self.y += 1.0;
    }
}
