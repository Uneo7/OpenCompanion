use std::sync::Arc;

use eframe::egui::{
    text::{LayoutJob, TextFormat, TextWrapping},
    Align, Color32, Context, FontFamily, FontId, Galley, Painter, Pos2, Rect, Vec2,
};

const TRADEMARK: char = '®';

/// `sup.tm`: the sign at .62 em in the regular weight, on the top of the line, .05 em after the word.
fn mark_format(f: &TextFormat) -> TextFormat {
    let family = match &f.font_id.family {
        FontFamily::Name(n) if n.starts_with("inter") => FontFamily::Name("inter-regular".into()),
        other => other.clone(),
    };
    TextFormat {
        font_id: FontId::new(f.font_id.size * 0.62, family),
        extra_letter_spacing: f.extra_letter_spacing * 0.62,
        valign: Align::TOP,
        ..f.clone()
    }
}

fn append(job: &mut LayoutJob, s: &str, format: TextFormat) {
    let mut rest = s;
    while let Some(i) = rest.find(TRADEMARK) {
        if i > 0 {
            job.append(&rest[..i], 0.0, format.clone());
        }
        job.append("®", 0.05 * format.font_id.size, mark_format(&format));
        rest = &rest[i + TRADEMARK.len_utf8()..];
    }
    if !rest.is_empty() {
        job.append(rest, 0.0, format);
    }
}

fn job_of(s: &str, format: TextFormat) -> LayoutJob {
    let mut job = LayoutJob::default();
    append(&mut job, s, format);
    job
}

#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub font: FontId,
    pub tracking: f32,
    pub upper: bool,
}

impl Style {
    pub fn new(font: FontId) -> Self {
        Self { font, tracking: -0.005, upper: false }
    }

    pub fn tracking(mut self, em: f32) -> Self {
        self.tracking = em;
        self
    }

    pub fn upper(mut self) -> Self {
        self.upper = true;
        self
    }

    fn format(&self, color: Color32, extra: f32) -> TextFormat {
        TextFormat {
            font_id: self.font.clone(),
            extra_letter_spacing: self.em_spacing() * self.font.size + extra,
            color,
            ..Default::default()
        }
    }

    /// Letter-spacing in em, including the optical-size compensation: the mockup uses Inter's variable
    /// `opsz` axis, whose display cuts (titles, 18 px and up) are a little tighter than the static text cut
    /// embedded here.
    ///
    /// egui does not apply Inter's kerning (GPOS only), which makes its lines about 1 % longer than the
    /// browser's; an average tightening of .007 em compensates (Inter only, Geist Mono is not kerned).
    fn em_spacing(&self) -> f32 {
        let inter = matches!(&self.font.family, eframe::egui::FontFamily::Name(n) if n.starts_with("inter"));
        if !inter {
            return self.tracking;
        }
        let display = (18.0..=32.0).contains(&self.font.size) && self.tracking < 0.0;
        self.tracking - 0.007 + if display { -0.011 } else { 0.0 }
    }

    /// Extra letter-spacing that gives `s` the width it has at the exact font size.
    ///
    /// egui rasterises and advances glyphs at a whole number of pixels for the font's line height
    /// (`size × (ascent − descent) / em`, rounded), so a 13 px Inter is laid out as if it were 13.2 px.
    /// The browser does not round; spreading the difference over the glyphs keeps line widths (and so
    /// wrapping and alignment) as in the mockup.
    fn size_correction(&self, ctx: &Context, s: &str) -> f32 {
        let n = s.chars().count();
        if n < 2 {
            return 0.0;
        }
        let ppp = ctx.pixels_per_point();
        let k = ctx.fonts(|f| f.row_height(&FontId::new(100.0, self.font.family.clone()))) / 100.0;
        let px = self.font.size * k * ppp;
        let rounded = px.round().max(1.0);
        let c = px / rounded;
        if (c - 1.0).abs() < 1e-3 {
            return 0.0;
        }
        let mut job = LayoutJob::single_section(s.to_owned(), self.format(Color32::WHITE, 0.0));
        job.wrap = TextWrapping::no_max_width();
        let w = ctx.fonts(|f| f.layout_job(job)).size().x;
        let natural = w - self.em_spacing() * self.font.size * (n - 1) as f32;
        natural / n as f32 * (c - 1.0)
    }

    fn text<'a>(&self, s: &'a str) -> std::borrow::Cow<'a, str> {
        if self.upper {
            s.to_uppercase().into()
        } else {
            s.into()
        }
    }
}

pub fn galley(ctx: &Context, s: &str, style: &Style, color: Color32) -> Arc<Galley> {
    let text = style.text(s);
    let extra = style.size_correction(ctx, &text);
    let mut job = job_of(&text, style.format(color, extra));
    job.wrap = TextWrapping::no_max_width();
    ctx.fonts(|f| f.layout_job(job))
}

pub fn width(ctx: &Context, s: &str, style: &Style) -> f32 {
    galley(ctx, s, style, Color32::WHITE).size().x
}

/// Paints one line in the CSS line box `[top, top + line_h]`, anchored at `x` (left, centre or right).
#[allow(clippy::too_many_arguments)]
pub fn line(painter: &Painter, s: &str, style: &Style, color: Color32, x: f32, top: f32, line_h: f32, align: Align) -> Rect {
    let g = galley(painter.ctx(), s, style, color);
    place(painter, g, x, top, line_h, align)
}

pub fn place(painter: &Painter, g: Arc<Galley>, x: f32, top: f32, line_h: f32, align: Align) -> Rect {
    let size = g.size();
    let left = match align {
        Align::Min => x,
        Align::Center => x - size.x / 2.0,
        Align::Max => x - size.x,
    };
    let pos = Pos2::new(left, top + (line_h - size.y) / 2.0);
    let rect = Rect::from_min_size(pos, size);
    painter.galley(pos, g, Color32::WHITE);
    rect
}

/// Distance from the top of a line box to the baseline (for baseline alignment of mixed sizes).
pub fn baseline_in_box(ctx: &Context, style: &Style, line_h: f32) -> f32 {
    let g = galley(ctx, "0", style, Color32::WHITE);
    let ascent = g.rows.first().and_then(|r| r.glyphs.first()).map_or(g.size().y * 0.8, |gl| gl.pos.y);
    (line_h - g.size().y) / 2.0 + ascent
}

pub fn paragraph(painter: &Painter, s: &str, style: &Style, color: Color32, min: Pos2, width: f32, line_h: f32) -> f32 {
    let g = paragraph_galley(painter.ctx(), s, style, color, width, line_h);
    let natural = painter.ctx().fonts(|f| f.row_height(&style.font));
    let h = g.rows.len() as f32 * line_h;
    painter.galley(min + Vec2::new(0.0, (line_h - natural) / 2.0), g, Color32::WHITE);
    h
}

pub fn paragraph_galley(ctx: &Context, s: &str, style: &Style, color: Color32, width: f32, line_h: f32) -> Arc<Galley> {
    let text = style.text(s);
    let extra = style.size_correction(ctx, &text);
    let mut format = style.format(color, extra);
    format.line_height = Some(line_h);
    let mut job = job_of(&text, format);
    job.wrap = TextWrapping::wrap_at_width(width);
    ctx.fonts(|f| f.layout_job(job))
}

pub fn rich_paragraph(painter: &Painter, runs: &[(&str, &Style, Color32)], min: Pos2, width: f32, line_h: f32) -> f32 {
    let mut job = LayoutJob::default();
    for (s, style, color) in runs {
        let text = style.text(s);
        let extra = style.size_correction(painter.ctx(), &text);
        let mut format = style.format(*color, extra);
        format.line_height = Some(line_h);
        append(&mut job, &text, format);
    }
    job.wrap = TextWrapping::wrap_at_width(width);
    let g = painter.ctx().fonts(|f| f.layout_job(job));
    let natural = runs.first().map_or(line_h, |r| painter.ctx().fonts(|f| f.row_height(&r.1.font)));
    let h = g.rows.len() as f32 * line_h;
    painter.galley(min + Vec2::new(0.0, (line_h - natural) / 2.0), g, Color32::WHITE);
    h
}

pub fn paragraph_height(ctx: &Context, s: &str, style: &Style, width: f32, line_h: f32) -> f32 {
    paragraph_galley(ctx, s, style, Color32::WHITE, width, line_h).rows.len() as f32 * line_h
}

pub mod st {
    use super::Style;
    use crate::theme::typography::{inter, mono, W};

    pub fn inter_(size: f32, w: W) -> Style {
        Style::new(inter(size, w))
    }

    pub fn mono_(size: f32, w: W) -> Style {
        Style::new(mono(size, w)).tracking(0.0)
    }

    pub fn sec_t() -> Style {
        inter_(12.0, W::Medium).tracking(0.01)
    }

    pub fn body(w: W) -> Style {
        inter_(14.0, w)
    }

    pub fn sec(w: W) -> Style {
        inter_(13.0, w)
    }

    pub fn page() -> Style {
        Style::new(inter(26.0, W::SemiBold)).tracking(-0.025)
    }

    pub fn pane() -> Style {
        Style::new(inter(20.0, W::SemiBold)).tracking(-0.015)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trademarks_are_superscripts() {
        let f = TextFormat { font_id: FontId::new(20.0, FontFamily::Name("inter-semibold".into())), ..Default::default() };
        let job = job_of("Bose® Companion® 5", f);
        let texts: Vec<_> = job.sections.iter().map(|s| &job.text[s.byte_range.clone()]).collect();
        assert_eq!(texts, ["Bose", "®", " Companion", "®", " 5"]);
        let mark = &job.sections[1].format;
        assert_eq!((mark.font_id.size, mark.valign), (20.0 * 0.62, Align::TOP));
        assert_eq!(mark.font_id.family, FontFamily::Name("inter-regular".into()));
        assert_eq!(job_of("plain", TextFormat::default()).sections.len(), 1);
    }
}
