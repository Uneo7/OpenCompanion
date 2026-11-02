use eframe::egui::{
    self,
    epaint::{Mesh, PathStroke, Vertex, WHITE_UV},
    Align, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2,
};

use super::reference::{self, Reference};
use super::{Flags, Geo, State};
use crate::model::biquad::{self, FREQS, POINTS};
use crate::model::eq::{Band, BandType};
use crate::i18n::{t_with, tr};
use crate::widgets::fmt::{self, tr_type};
use crate::theme::{self, typography::W, Theme};
use crate::widgets::{paint as wp, text, text::st};

/// Curves are clamped a little beyond the visible range.
pub(super) fn curve_points(geo: &Geo, values: &[f64; POINTS]) -> Vec<Pos2> {
    let clamp = geo.range + 3.0;
    FREQS.iter().zip(values).map(|(f, v)| Pos2::new(geo.x_of(*f), geo.y_of(v.clamp(-clamp, clamp)))).collect()
}

fn fill_to_zero(painter: &Painter, geo: &Geo, pts: &[Pos2], top: impl Fn(Pos2) -> Color32, bottom: impl Fn(Pos2) -> Color32) {
    let y0 = geo.y_of(0.0);
    let mut mesh = Mesh::default();
    for p in pts {
        let b = Pos2::new(p.x, y0);
        mesh.vertices.push(Vertex { pos: *p, uv: WHITE_UV, color: top(*p) });
        mesh.vertices.push(Vertex { pos: b, uv: WHITE_UV, color: bottom(b) });
    }
    for i in 0..pts.len().saturating_sub(1) as u32 {
        let (a, b, c, d) = (2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 3);
        mesh.add_triangle(a, b, c);
        mesh.add_triangle(b, d, c);
    }
    painter.add(Shape::mesh(mesh));
}

fn text_baseline(painter: &Painter, s: &str, style: &text::Style, color: Color32, x: f32, y: f32, align: Align) {
    let g = text::galley(painter.ctx(), s, style, color);
    let ascent = g.rows.first().and_then(|r| r.glyphs.first()).map_or(g.size().y * 0.78, |gl| gl.pos.y);
    let w = g.size().x;
    let left = match align {
        Align::Min => x,
        Align::Center => x - w / 2.0,
        Align::Max => x - w,
    };
    painter.galley(Pos2::new(left, y - ascent), g, Color32::WHITE);
}

fn grid(painter: &Painter, geo: &Geo, t: &Theme) {
    let (x0, x1, y0, y1) = (geo.x0(), geo.x1(), geo.y0(), geo.y1());
    let px = |x: f32| x.round() + 0.5;
    let major = [20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0, 20_000.0];
    let mut dec = 10.0;
    while dec <= 10_000.0 {
        for k in 2..10 {
            let f = dec * f64::from(k);
            if !(20.0..=20_000.0).contains(&f) || [20.0, 50.0, 200.0, 500.0, 2000.0, 5000.0].contains(&f) {
                continue;
            }
            let x = px(geo.x_of(f));
            painter.line_segment([Pos2::new(x, y0), Pos2::new(x, y1)], Stroke::new(1.0_f32, t.grid[0]));
        }
        dec *= 10.0;
    }
    let label = st::mono_(11.0, W::Medium);
    let h = geo.canvas.bottom();
    for f in major {
        let x = px(geo.x_of(f));
        painter.line_segment([Pos2::new(x, y0), Pos2::new(x, y1)], Stroke::new(1.0_f32, t.grid[1]));
        let s = if f >= 1000.0 {
            format!("{}k{}", f / 1000.0, if f == 20_000.0 { " Hz" } else { "" })
        } else {
            format!("{f}")
        };
        let align = if f == 20.0 {
            Align::Min
        } else if f == 20_000.0 {
            Align::Max
        } else {
            Align::Center
        };
        text_baseline(painter, &s, &label, t.muted, x, h - 8.0, align);
    }
    // Gridlines every 6 dB (12 dB once the range is wider than ±18), the outer ones unlabelled.
    let r = geo.range;
    let step = if r > 18.0 { 12.0 } else { 6.0 };
    let n = (r / step).floor() as i32;
    for d in (-n..=n).map(|k| f64::from(k) * step) {
        let y = geo.y_of(d).round() + 0.5;
        let c = if d == 0.0 { t.grid[2] } else { t.grid[1] };
        painter.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], Stroke::new(1.0_f32, c));
        if d.abs() < r - 0.01 {
            let s = match d {
                0.0 => "0 dB".to_owned(),
                d if d > 0.0 => format!("+{d}"),
                d => format!("−{}", -d),
            };
            text_baseline(painter, &s, &label, t.muted, x0 + 4.0, y - 5.0, Align::Min);
        }
    }
}

pub(crate) struct Scene<'a> {
    pub bands: &'a [Band],
    pub base: &'a [Band],
    pub selected: usize,
    pub flags: Flags,
    /// Curve-morph progress (1 = settled) and the curve it starts from.
    pub morph: Option<(f32, &'a [f64; POINTS])>,
    pub reference: Option<Reference<'a>>,
}

pub(crate) fn drawn_curve(scene: &Scene) -> [f64; POINTS] {
    let chain: Vec<Band> = scene.base.iter().chain(scene.bands.iter()).copied().collect();
    let user = if scene.flags.bypass { [0.0; POINTS] } else { biquad::response(&chain) };
    match scene.morph {
        Some((k, from)) if k < 1.0 => std::array::from_fn(|i| from[i] + (user[i] - from[i]) * f64::from(k)),
        _ => user,
    }
}

pub(crate) fn paint(ui: &egui::Ui, geo: &Geo, scene: &Scene, user: &[f64; POINTS], st: &State) {
    let t = theme::get(ui.ctx());
    let painter = ui.painter();
    let flags = scene.flags;
    let light = t.light;
    grid(painter, geo, &t);

    let user_pts = curve_points(geo, user);
    let clip = Rect::from_min_max(Pos2::new(geo.x0(), geo.y0() - 4.0), Pos2::new(geo.x1(), geo.y1() + 4.0));
    let cp = painter.with_clip_rect(clip.intersect(painter.clip_rect()));
    let dragging = st.drag.is_some();
    let mk = scene.morph.map_or(1.0, |(k, _)| k);
    let lerp = |a: f32, b: f32| a + (b - a) * light;

    // The selected band's own shape, faint, in its colour (the others are not drawn: one curve to read).
    if !flags.compare && !flags.bypass && !flags.reference_only {
        let bp = wp::faded(&cp, mk);
        for (i, b) in scene.bands.iter().enumerate().filter(|(i, b)| b.on && *i == scene.selected) {
            let hu = t.band(i);
            let sel = i == scene.selected;
            let one = biquad::band_response(b);
            let pts = curve_points(geo, &one);
            let fill = theme::alpha(hu, if sel { lerp(0.16, 0.24) } else { lerp(0.06, 0.10) });
            fill_to_zero(&bp, geo, &pts, |_| fill, |_| fill);
            let stroke = Stroke::new(if sel { 1.6_f32 } else { 1.2 }, theme::alpha(hu, if sel { 0.9 } else { lerp(0.3, 0.55) }));
            let mut run: Vec<Pos2> = Vec::new();
            for (k, v) in one.iter().enumerate() {
                if v.abs() > 0.15 {
                    run.push(pts[k]);
                } else if !run.is_empty() {
                    let r = std::mem::take(&mut run);
                    if r.len() > 1 {
                        bp.add(Shape::line(r, stroke));
                    }
                }
            }
            if run.len() > 1 {
                bp.add(Shape::line(run, stroke));
            }
        }
    }

    if let (Some((f, d)), None, false) = (st.hover, st.hover_node, dragging) {
        if !flags.compare && !flags.reference_only && flags.can_add {
            let pts = curve_points(geo, &biquad::band_response(&Band::new(BandType::Peak, f, d, 1.0)));
            cp.extend(Shape::dashed_line(&pts, Stroke::new(1.0_f32, theme::alpha(t.text, 0.35)), 3.0, 4.0));
        }
    }

    // The real Bose response of the output, under the EQ curve: a thin, subtle background reference (the only
    // curve, at full strength, on the locked profile). No result curve.
    if let Some(r) = &scene.reference {
        reference::paint_curves(&cp, geo, &t, r.curve, flags.compare, !flags.reference_only);
    }

    let (c1, c2) = (t.curve1, t.curve2);
    if flags.reference_only {
        if let Some(r) = &scene.reference {
            reference::paint_legend(ui, geo, &t, r);
        }
        return;
    }

    if !flags.compare {
        let (ytop, yzero) = (geo.y_of(geo.range), geo.y_of(0.0));
        let top_a = lerp(0.10, 0.20);
        let grad = |p: Pos2| {
            let k = ((p.y - ytop) / (yzero - ytop)).clamp(0.0, 1.0);
            theme::mix(theme::alpha(c1, top_a), theme::alpha(c2, 0.0), k)
        };
        fill_to_zero(&cp, geo, &user_pts, grad, grad);
    }

    let (x0, x1) = (geo.x0(), geo.x1());
    let alpha = if flags.compare { 0.35 } else { 1.0 };
    let stroke = PathStroke::new_uv(2.6_f32, move |_r, p: Pos2| theme::alpha(theme::mix(c1, c2, ((p.x - x0) / (x1 - x0)).clamp(0.0, 1.0)), alpha));
    cp.add(Shape::line(user_pts, stroke));

    if !flags.compare {
        points(&wp::faded(painter, mk), geo, scene, user, st, &t);
    }
    overlay_text(painter, geo, scene.bands, flags, st, &t);
    if let Some(r) = &scene.reference {
        reference::paint_legend(ui, geo, &t, r);
    }
}

fn points(painter: &Painter, geo: &Geo, scene: &Scene, user: &[f64; POINTS], st: &State, t: &Theme) {
    let num = st::mono_(11.0, W::SemiBold);
    for (i, b) in scene.bands.iter().enumerate() {
        let p = geo.node(b);
        let sel = i == scene.selected;
        let hov = st.hover_node == Some(i);
        let hu = t.band(i);
        let yc = geo.y_of(biquad::sample_at(user, b.freq).clamp(-(geo.range + 3.0), geo.range + 3.0));
        if b.on && (yc - p.y).abs() > 12.0 {
            let from = Pos2::new(p.x, p.y + if yc > p.y { 10.0 } else { -10.0 });
            painter.extend(Shape::dashed_line(&[from, Pos2::new(p.x, yc)], Stroke::new(1.0_f32, theme::alpha(hu, 0.5)), 2.0, 3.0));
            painter.circle_filled(Pos2::new(p.x, yc), 2.5, hu);
        }
        if sel {
            painter.circle_filled(p, 15.0, theme::alpha(hu, 0.2));
        }
        painter.circle_filled(p, 10.0, if sel { hu } else { t.surface_solid });
        let w = if hov || sel { 2.0_f32 } else { 1.5 };
        if b.on {
            painter.circle_stroke(p, 10.0, Stroke::new(w, hu));
        } else {
            wp::dashed_circle(painter, p, 10.0, Stroke::new(w, t.faint), 2.0, 2.0);
        }
        let c = if sel {
            t.on_fill
        } else if b.on {
            t.text
        } else {
            t.faint
        };
        text_baseline(painter, &(i + 1).to_string(), &num, c, p.x, p.y + 4.0, Align::Center);
    }
}

fn overlay_text(painter: &Painter, geo: &Geo, bands: &[Band], flags: Flags, st: &State, t: &Theme) {
    let (x0, x1, y0, y1) = (geo.x0(), geo.x1(), geo.y0(), geo.y1());
    let dragging = st.drag.is_some();
    if let (Some(i), false, false) = (st.hover_node, dragging, flags.compare) {
        if let Some(b) = bands.get(i) {
            let mut s = format!("{} · {} · {}", t_with("eq.band", &[("n", &(i + 1))]), tr_type(b.kind), fmt::freq(b.freq));
            if b.kind.has_gain() {
                s += &format!(" · {} dB", fmt::frs(b.gain, 1));
            }
            s += &format!(" · Q {}", fmt::fr(b.q, 2));
            if !b.on {
                s += &format!(" · {}", tr("eq.band.off"));
            }
            let style = st::inter_(12.0, W::Medium).tracking(0.0);
            let tw = text::width(painter.ctx(), &s, &style) + 20.0;
            let n = geo.node(b);
            let x = (n.x - tw / 2.0).clamp(x0, x1 - tw);
            let y = if n.y - 44.0 < y0 { n.y + 16.0 } else { n.y - 42.0 };
            let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(tw, 26.0));
            painter.rect(r, 7.0, t.pop, Stroke::new(1.0_f32, t.line_strong), egui::StrokeKind::Inside);
            text_baseline(painter, &s, &style, t.text, x + 10.0, y + 17.0, Align::Min);
        }
    } else if let (Some((f, d)), false, false) = (st.hover, dragging, flags.compare) {
        let s = format!("{}  {} dB", fmt::freq(f), fmt::frs(d, 1));
        text_baseline(painter, &s, &st::mono_(11.0, W::Medium), t.muted, x1 - 8.0, y1 - 8.0, Align::Max);
    }
    if flags.compare || flags.bypass {
        let note = tr(if flags.compare { "eq.note.ab" } else { "eq.note.bypass" });
        text::line(painter, note, &st::sec(W::Regular), t.muted, geo.canvas.center().x, geo.canvas.top() + 12.0, 18.0, Align::Center);
    }
}
