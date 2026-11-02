use eframe::egui::{self, Id, Pos2};

use super::docs::Doc;
use crate::assets::icons;
use crate::i18n::tr;
use crate::theme::{self, typography::W};
use crate::views::settings::rows::Page;
use crate::widgets::{button, text, text::st};

pub fn show(ui: &mut egui::Ui, p: &mut Page, doc: Doc, max_w: f32) -> bool {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let head_top = p.y;
    p.head(ui, doc.title(), None);
    let back = button::Button::new(tr("settings.legal.title")).icon(icons::BACK).small().kind(button::Kind::Quiet);
    let width = p.width().min(max_w);
    let bw = back.size(&ctx).x;
    let clicked = back.show(ui, Pos2::new(p.left + width - bw, head_top + 4.0), Id::new("legal-back")).clicked();
    let body = st::mono_(12.0, W::Regular);
    p.y += 16.0;
    p.y += text::paragraph(ui.painter(), doc.text().trim_end(), &body, t.muted, Pos2::new(p.left, p.y), width, 18.0);
    clicked
}
