mod geometry;
mod input;
mod paint;
mod reference;

pub use geometry::{fit_range, Geo, RANGE};
pub use reference::Reference;

use eframe::egui::{self, Id, Pos2, Rect};

use crate::model::biquad::{self, POINTS};
use crate::model::eq::Band;
use crate::theme::motion;

#[derive(Debug, Clone, Copy, Default)]
pub struct Flags {
    /// Listening to the factory sound: points hidden, the EQ curve faded.
    pub compare: bool,
    pub bypass: bool,
    /// Room for another band (hover preview of a new bell).
    pub can_add: bool,
    /// Identifies what is shown (target, profile): a change morphs the curve from the previous one.
    pub morph_key: u64,
    /// Read-only profile: only the reference curve is drawn; no band can be added or edited.
    pub reference_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Select,
    Value,
    Struct,
}

#[derive(Debug, Default)]
pub struct Output {
    pub change: Option<Change>,
    pub context: Option<(usize, Pos2)>,
    pub add: Option<(f64, f64)>,
}

impl Output {
    fn set(&mut self, c: Change) {
        let rank = |c: Change| match c {
            Change::Select => 0,
            Change::Value => 1,
            Change::Struct => 2,
        };
        if self.change.is_none_or(|old| rank(c) > rank(old)) {
            self.change = Some(c);
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct State {
    pub hover: Option<(f64, f64)>,
    pub hover_node: Option<usize>,
    pub drag: Option<input::Drag>,
}

/// Curve morph between targets (~260 ms, cubic ease-out): the curve drawn last frame, and the morph in flight.
#[derive(Clone, Default)]
struct Morph {
    key: u64,
    last: Vec<f64>,
    from: Vec<f64>,
    last_ref: Vec<f64>,
    from_ref: Vec<f64>,
    t0: f64,
}

const MORPH_S: f64 = 0.26;

/// `base` holds the bands that precede `bands` in the chain (the global list, when a speaker is targeted);
/// `reference` is the real response of the profile's sound base for the shown target.
#[allow(clippy::too_many_arguments)]
pub fn eq_graph(
    ui: &mut egui::Ui,
    rect: Rect,
    id: Id,
    bands: &mut [Band],
    base: &[Band],
    selected: &mut usize,
    flags: Flags,
    reference: Option<Reference>,
) -> Output {
    let ctx = ui.ctx().clone();
    // Gain range: widened (smoothly) when the curve goes beyond ±12 dB, back to ±12 when it fits again. The
    // base and result curves count by their peaks (their crossover stop bands leave the plot at the bottom).
    let target = {
        let chain: Vec<Band> = base.iter().chain(bands.iter()).copied().collect();
        let user = biquad::response(&chain);
        let mut r = if flags.bypass { RANGE } else { fit_range(&user) };
        if let Some(rf) = &reference {
            r = r.max(reference::fit_peaks(rf.curve));
            if let (false, false, Some(sum)) = (flags.bypass, flags.compare, reference::result(rf.curve, &user)) {
                r = r.max(reference::fit_peaks(&sum));
            }
        }
        r
    };
    let rid = id.with("range");
    let shown: f64 = ctx.data(|d| d.get_temp(rid)).unwrap_or(target);
    let range = if motion::reduced(&ctx) || (shown - target).abs() < 0.05 { target } else { shown + (target - shown) * 0.2 };
    if range != target {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(rid, range));
    let geo = Geo::new(rect).with_range(range);
    let mut state: State = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
    let out = input::handle(ui, id, &geo, bands, selected, flags, &mut state);

    // Morph: when the target changes, the drawn curve starts from the one drawn last frame.
    let mid = id.with("morph");
    let mut m: Morph = ctx.data(|d| d.get_temp(mid)).unwrap_or_default();
    let now = ctx.input(|i| i.time);
    if m.key != flags.morph_key && !m.last.is_empty() && !motion::reduced(&ctx) {
        m.from = m.last.clone();
        m.from_ref = m.last_ref.clone();
        m.t0 = now;
    }
    m.key = flags.morph_key;
    let p = if m.from.len() == POINTS { ((now - m.t0) / MORPH_S).clamp(0.0, 1.0) as f32 } else { 1.0 };
    let k = 1.0 - (1.0 - p).powi(3);
    if p < 1.0 {
        ctx.request_repaint();
    }
    let from: [f64; POINTS] = if m.from.len() == POINTS { std::array::from_fn(|i| m.from[i]) } else { [0.0; POINTS] };
    let ref_curve: Option<[f64; POINTS]> = reference.map(|r| match (p < 1.0, m.from_ref.len() == POINTS) {
        (true, true) => std::array::from_fn(|i| m.from_ref[i] + (r.curve[i] - m.from_ref[i]) * f64::from(k)),
        _ => *r.curve,
    });
    let scene = paint::Scene {
        bands,
        base,
        selected: *selected,
        flags,
        morph: (p < 1.0).then_some((k, &from)),
        reference: reference.zip(ref_curve.as_ref()).map(|(r, c)| Reference { curve: c, ..r }),
    };
    let user = paint::drawn_curve(&scene);
    paint::paint(ui, &geo, &scene, &user, &state);
    m.last = user.to_vec();
    m.last_ref = ref_curve.map_or_else(Vec::new, |c| c.to_vec());
    ctx.data_mut(|d| {
        d.insert_temp(id, state);
        d.insert_temp(mid, m);
    });
    out
}
