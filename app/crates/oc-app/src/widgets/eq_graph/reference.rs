use eframe::egui::{self, Align, Painter, Pos2, Rect, Sense, Shape, Stroke};

use super::geometry::{Geo, RANGE, RANGE_MAX};
use crate::model::biquad::POINTS;
use crate::theme::{typography::W, Theme};
use crate::widgets::{text, text::st, tooltip};

#[derive(Clone, Copy)]
pub struct Reference<'a> {
    pub curve: &'a [f64; POINTS],
    pub label: &'a str,
    pub tip: &'a str,
}

const HELP_W: f32 = 30.0;
/// The user curve counts as flat below this (dB): no result curve then (it would be the base curve).
const FLAT: f64 = 0.05;

/// Range that shows the peaks of a base or result curve: their positive excursion only. Their deep stop
/// bands (crossover, -80 dB) leave the plot at the bottom, as the EQ curve's own cuts beyond the range do.
pub fn fit_peaks(curve: &[f64]) -> f64 {
    let peak = curve.iter().fold(0.0_f64, |m, v| m.max(*v));
    (((peak + 1.5) / 6.0).ceil() * 6.0).clamp(RANGE, RANGE_MAX)
}

pub fn result(base: &[f64; POINTS], user: &[f64; POINTS]) -> Option<[f64; POINTS]> {
    user.iter().any(|v| v.abs() > FLAT).then(|| std::array::from_fn(|i| base[i] + user[i]))
}

fn points(geo: &Geo, values: &[f64; POINTS]) -> Vec<Pos2> {
    super::paint::curve_points(geo, values)
}

/// The Bose curve, solid in the text colour: thin and faint as a background reference (`subtle`), stronger while
/// comparing or when it is the only curve (the locked profile).
pub(crate) fn paint_curves(cp: &Painter, geo: &Geo, t: &Theme, base: &[f64; POINTS], compare: bool, subtle: bool) {
    let stroke = match (compare, subtle) {
        (true, _) => Stroke::new(2.2_f32, t.text),
        (false, true) => Stroke::new(1.0_f32, crate::theme::alpha(t.text, 0.35)),
        (false, false) => Stroke::new(1.5_f32, t.text),
    };
    cp.add(Shape::line(points(geo, base), stroke));
}

pub(crate) fn paint_legend(ui: &egui::Ui, geo: &Geo, t: &Theme, r: &Reference) {
    let painter = ui.painter();
    let style = st::inter_(12.0, W::Medium).tracking(0.0);
    let (h, sample, gap) = (22.0_f32, 16.0_f32, 6.0_f32);
    let top = geo.canvas.top() + 4.0;
    let end = geo.canvas.right() - HELP_W - 12.0;
    let mut right = end;
    let mut item = |label: &str, dashed: bool| {
        let tw = text::width(ui.ctx(), label, &style);
        text::line(painter, label, &style, t.muted, right, top, h, Align::Max);
        let x1 = right - tw - gap;
        let y = top + h / 2.0;
        let (a, b) = (Pos2::new(x1 - sample, y), Pos2::new(x1, y));
        if dashed {
            painter.extend(Shape::dashed_line(&[a, b], Stroke::new(1.8_f32, t.curve1), 4.0, 3.0));
        } else {
            painter.line_segment([a, b], Stroke::new(1.5_f32, t.text));
        }
        let left = a.x;
        right = left - 14.0;
        left
    };
    let left = item(r.label, false);
    let area = Rect::from_min_max(Pos2::new(left, top), Pos2::new(end, top + h));
    let resp = ui.interact(area, ui.id().with("eq-ref-legend"), Sense::hover());
    tooltip::attach(&resp, r.tip, None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaks_only_widen_for_positive_excursions() {
        let mut c = [-80.0; POINTS];
        assert_eq!(fit_peaks(&c), RANGE);
        c[10] = 15.5;
        assert_eq!(fit_peaks(&c), 18.0);
    }

    #[test]
    fn no_result_curve_for_a_flat_eq() {
        let base = [3.0; POINTS];
        assert!(result(&base, &[0.0; POINTS]).is_none());
        let mut user = [0.0; POINTS];
        user[5] = 1.0;
        assert_eq!(result(&base, &user).unwrap()[5], 4.0);
    }
}
