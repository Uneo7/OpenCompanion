use eframe::egui::{Pos2, Rect};

use crate::model::biquad::{F_MAX, F_MIN};
use crate::model::eq::Band;

/// Default visible gain range (± dB); the graph widens it when the curve goes beyond (`fit_range`).
pub const RANGE: f64 = 12.0;
pub const RANGE_MAX: f64 = 36.0;

/// Range that shows the whole curve: ±12 dB, widened by 6 dB steps with 1.5 dB of headroom, at most ±36.
pub fn fit_range(curve: &[f64]) -> f64 {
    let peak = curve.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    (((peak + 1.5) / 6.0).ceil() * 6.0).clamp(RANGE, RANGE_MAX)
}
const TOP: f32 = 12.0;
const BOTTOM: f32 = 26.0;

#[derive(Debug, Clone, Copy)]
pub struct Geo {
    pub canvas: Rect,
    pub range: f64,
}

impl Geo {
    pub fn new(canvas: Rect) -> Self {
        Self { canvas, range: RANGE }
    }

    pub fn with_range(mut self, range: f64) -> Self {
        self.range = range;
        self
    }

    pub fn x0(&self) -> f32 {
        self.canvas.left()
    }

    pub fn x1(&self) -> f32 {
        self.canvas.right()
    }

    pub fn y0(&self) -> f32 {
        self.canvas.top() + TOP
    }

    pub fn y1(&self) -> f32 {
        self.canvas.bottom() - BOTTOM
    }

    pub fn x_of(&self, f: f64) -> f32 {
        self.x0() + ((f / F_MIN).ln() / (F_MAX / F_MIN).ln()) as f32 * (self.x1() - self.x0())
    }

    pub fn f_of(&self, x: f32) -> f64 {
        F_MIN * (F_MAX / F_MIN).powf(f64::from((x - self.x0()) / (self.x1() - self.x0())))
    }

    pub fn y_of(&self, d: f64) -> f32 {
        self.y0() + ((self.range - d) / (2.0 * self.range)) as f32 * (self.y1() - self.y0())
    }

    pub fn d_of(&self, y: f32) -> f64 {
        self.range - f64::from((y - self.y0()) / (self.y1() - self.y0())) * 2.0 * self.range
    }

    pub fn node(&self, b: &Band) -> Pos2 {
        Pos2::new(self.x_of(b.freq), self.y_of(b.shown_gain().clamp(-self.range, self.range)))
    }

    /// Index of the point within 16 px of `p` (the closest one).
    pub fn hit(&self, bands: &[Band], p: Pos2) -> Option<usize> {
        let mut best = None;
        let mut bd = 16.0;
        for (i, b) in bands.iter().enumerate() {
            let d = self.node(b).distance(p);
            if d < bd {
                bd = d;
                best = Some(i);
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::eq::BandType;
    use eframe::egui::Vec2;

    fn geo() -> Geo {
        Geo::new(Rect::from_min_size(Pos2::new(90.0, 204.0), Vec2::new(920.0, 318.0)))
    }

    #[test]
    fn mapping_round_trips() {
        let g = geo();
        assert_eq!(g.x_of(20.0), 90.0);
        assert!((g.x_of(20_000.0) - 1010.0).abs() < 1e-3);
        assert!((g.f_of(g.x_of(1234.0)) - 1234.0).abs() < 0.01);
        assert_eq!(g.y_of(12.0), 216.0);
        assert_eq!(g.y_of(-12.0), 496.0);
        assert!((g.d_of(g.y_of(-3.3)) + 3.3).abs() < 1e-4);
    }

    #[test]
    fn hit_picks_the_closest_point_within_16_px() {
        let g = geo();
        let bands = [Band::new(BandType::Peak, 1000.0, 0.0, 1.0), Band::new(BandType::Peak, 1100.0, 0.0, 1.0)];
        let p1 = g.node(&bands[1]);
        assert_eq!(g.hit(&bands, p1 + Vec2::new(1.0, 0.0)), Some(1));
        assert_eq!(g.hit(&bands, p1 + Vec2::new(0.0, 30.0)), None);
        // Shapes without gain sit on the 0 dB line.
        let hp = Band::new(BandType::HighPass, 30.0, 9.0, 0.7);
        assert_eq!(g.node(&hp).y, g.y_of(0.0));
    }
}
