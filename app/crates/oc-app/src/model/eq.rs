use super::biquad::{F_MAX, F_MIN};

pub const MAX_BANDS: usize = 12;
/// Gain limit of a band (dB, both directions).
pub const GAIN_LIMIT: f64 = 15.0;
pub const Q_MIN: f64 = 0.1;
pub const Q_MAX: f64 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandType {
    Peak,
    LowShelf,
    HighShelf,
    HighPass,
    LowPass,
    Notch,
}

impl BandType {
    /// In the order of the band panel and the context menu.
    pub const ALL: [BandType; 6] =
        [BandType::Peak, BandType::LowShelf, BandType::HighShelf, BandType::HighPass, BandType::LowPass, BandType::Notch];

    pub fn has_gain(self) -> bool {
        matches!(self, BandType::Peak | BandType::LowShelf | BandType::HighShelf)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    pub kind: BandType,
    /// Centre or corner frequency (Hz).
    pub freq: f64,
    /// Gain (dB); ignored by the pass and notch shapes.
    pub gain: f64,
    pub q: f64,
    pub on: bool,
}

impl Band {
    pub const fn new(kind: BandType, freq: f64, gain: f64, q: f64) -> Self {
        Self { kind, freq, gain, q, on: true }
    }

    pub const fn off(self) -> Self {
        Self { on: false, ..self }
    }

    pub fn created_at(freq: f64, gain: f64) -> Self {
        let kind = if freq < 30.0 {
            BandType::HighPass
        } else if freq > 16_000.0 {
            BandType::LowPass
        } else {
            BandType::Peak
        };
        let (gain, q) = if kind == BandType::Peak { (gain, 1.0) } else { (0.0, 0.71) };
        Self::new(kind, freq.round(), gain, q)
    }

    /// Gain as drawn and edited (0 for shapes without gain).
    pub fn shown_gain(&self) -> f64 {
        if self.kind.has_gain() {
            self.gain
        } else {
            0.0
        }
    }

    pub fn set_freq(&mut self, f: f64) {
        self.freq = f.clamp(F_MIN, F_MAX).round();
    }

    pub fn set_gain(&mut self, g: f64) {
        self.gain = (g.clamp(-GAIN_LIMIT, GAIN_LIMIT) * 10.0).round() / 10.0;
    }

    pub fn set_q(&mut self, q: f64) {
        self.q = (q.clamp(Q_MIN, Q_MAX) * 100.0).round() / 100.0;
    }
}

/// Where a list of bands applies: the DSP's six EQ targets, in their order (`EQ_GLOBAL`, `EQ_INNER_L`, …). The
/// editor has two modes: "Simple" ([`Target::Global`]) and "Advanced" (one of [`Target::ADVANCED`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    Global,
    InnerLeft,
    InnerRight,
    OuterLeft,
    OuterRight,
    Sub,
}

impl Target {
    pub const ALL: [Target; 6] =
        [Target::Global, Target::InnerLeft, Target::InnerRight, Target::OuterLeft, Target::OuterRight, Target::Sub];
    pub const ADVANCED: [Target; 5] = [Target::InnerLeft, Target::InnerRight, Target::OuterLeft, Target::OuterRight, Target::Sub];

    pub fn is_advanced(self) -> bool {
        self != Target::Global
    }

    pub fn mirror(self) -> Option<Target> {
        match self {
            Target::InnerLeft => Some(Target::InnerRight),
            Target::InnerRight => Some(Target::InnerLeft),
            Target::OuterLeft => Some(Target::OuterRight),
            Target::OuterRight => Some(Target::OuterLeft),
            _ => None,
        }
    }

    /// Index in [`Target::ALL`], which is the DSP target index.
    pub fn index(self) -> usize {
        self as usize
    }
}

/// Sound base of a profile (`PF_STOCK_GRAPH`): the stock Bose signal graph with the EQ on top, or the neutral
/// EQ path (`firmware/dsp/README.md`, "The graph is a per-profile base").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Base {
    /// Stock Bose graph (the firmware's default for a new or empty slot). The app writes every profile on it.
    #[default]
    Bose,
    /// EQ path only (profiles saved by older app versions; read as the Bose base, see `hw::convert`).
    Neutral,
}

impl Base {
    pub const ALL: [Base; 2] = [Base::Bose, Base::Neutral];
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProfileBands {
    pub base: Base,
    /// EQ mode, stored with the profile (`PF_ADVANCED`): "Simple" = the global list only; "Advanced" = one list
    /// per output and no global list.
    pub advanced: bool,
    lists: [Vec<Band>; 6],
}

impl ProfileBands {
    pub fn flat() -> Self {
        Self::default()
    }

    pub fn bose() -> Self {
        Self::default()
    }

    pub fn get(&self, t: Target) -> &Vec<Band> {
        &self.lists[t.index()]
    }

    pub fn get_mut(&mut self, t: Target) -> &mut Vec<Band> {
        &mut self.lists[t.index()]
    }

    /// Simple → Advanced: the global bands go in front of each output's list (the sound does not change), then
    /// the global list is cleared. Lists are cut at [`MAX_BANDS`].
    pub fn to_advanced(&mut self) {
        let global = std::mem::take(self.get_mut(Target::Global));
        for t in Target::ADVANCED {
            let mut l = global.clone();
            l.extend(self.get(t).iter().copied());
            l.truncate(MAX_BANDS);
            *self.get_mut(t) = l;
        }
        self.advanced = true;
    }

    pub fn to_simple(&mut self) {
        for l in &mut self.lists {
            l.clear();
        }
        self.advanced = false;
    }

    /// Mode of a profile read without the `PF_ADVANCED` bit (DSP firmware that masks it): Advanced when an
    /// output has bands.
    pub fn inferred_advanced(&self) -> bool {
        Target::ADVANCED.iter().any(|t| !self.get(*t).is_empty())
    }

    pub fn mirror_from(&mut self, t: Target) {
        if let Some(m) = t.mirror() {
            let src = self.get(t).clone();
            *self.get_mut(m) = src;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_modes_keeps_the_sound_then_starts_flat() {
        let mut p = ProfileBands::bose();
        p.get_mut(Target::Global).push(Band::new(BandType::Peak, 100.0, 3.0, 1.0));
        p.get_mut(Target::Sub).push(Band::new(BandType::LowPass, 120.0, 0.0, 0.71));
        p.to_advanced();
        assert!(p.advanced && p.get(Target::Global).is_empty());
        assert_eq!(p.get(Target::InnerLeft).len(), 1);
        assert_eq!((p.get(Target::Sub)[0].freq, p.get(Target::Sub)[1].freq), (100.0, 120.0));
        p.to_simple();
        assert!(!p.advanced && Target::ALL.iter().all(|t| p.get(*t).is_empty()));
    }

    #[test]
    fn setters_clamp_and_round() {
        let mut b = Band::new(BandType::Peak, 1000.0, 0.0, 1.0);
        b.set_freq(5.0);
        assert_eq!(b.freq, 20.0);
        b.set_freq(1234.56);
        assert_eq!(b.freq, 1235.0);
        b.set_gain(-20.0);
        assert_eq!(b.gain, -15.0);
        b.set_gain(1.234);
        assert_eq!(b.gain, 1.2);
        b.set_q(50.0);
        assert_eq!(b.q, 12.0);
        b.set_q(0.7051);
        assert_eq!(b.q, 0.71);
    }

    #[test]
    fn created_band_depends_on_where_you_click() {
        assert_eq!(Band::created_at(25.0, 3.0).kind, BandType::HighPass);
        assert_eq!(Band::created_at(18_000.0, 3.0).kind, BandType::LowPass);
        let b = Band::created_at(1000.4, -2.5);
        assert_eq!((b.kind, b.freq, b.gain, b.q), (BandType::Peak, 1000.0, -2.5, 1.0));
        assert_eq!(Band::created_at(25.0, 3.0).gain, 0.0);
    }

    #[test]
    fn lists_are_independent_per_target() {
        let mut p = ProfileBands::bose();
        p.get_mut(Target::Sub).push(Band::new(BandType::LowPass, 160.0, 0.0, 0.71));
        assert!(p.get(Target::Global).is_empty());
        assert_eq!(p.get(Target::Sub)[0].kind, BandType::LowPass);
        assert_eq!(ProfileBands::flat(), ProfileBands::default());
        assert!(Target::ALL.iter().enumerate().all(|(i, t)| t.index() == i));
    }

    #[test]
    fn mirror_copies_to_the_other_side_only() {
        let mut p = ProfileBands::flat();
        p.get_mut(Target::OuterLeft).push(Band::new(BandType::HighShelf, 7000.0, 1.5, 0.7));
        p.mirror_from(Target::OuterLeft);
        assert_eq!(p.get(Target::OuterRight), p.get(Target::OuterLeft));
        assert!(p.get(Target::InnerRight).is_empty());
        p.mirror_from(Target::Global);
        assert_eq!(Target::ALL.iter().map(|t| p.get(*t).len()).sum::<usize>(), 2);
    }

    #[test]
    fn only_bell_and_shelves_have_gain() {
        let with: Vec<_> = BandType::ALL.iter().filter(|t| t.has_gain()).collect();
        assert_eq!(with, [&BandType::Peak, &BandType::LowShelf, &BandType::HighShelf]);
        let mut b = Band::new(BandType::Notch, 50.0, 6.0, 10.0);
        assert_eq!(b.shown_gain(), 0.0);
        b.kind = BandType::Peak;
        assert_eq!(b.shown_gain(), 6.0);
    }
}
