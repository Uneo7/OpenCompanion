//! A base in memory: exactly the numbers the DSP runs (float32, 48 kHz, Bose section convention).

use super::topology::{SetId, DELAYS, SETS};

/// One section, Bose convention, monic: H(z) = (1 + b1 z⁻¹ + b2 z⁻²) / (1 − a1 z⁻¹ − a2 z⁻²).
/// Order `[b1, b2, a1, a2]` (as `biquad_coef_t`).
pub type Coefs = [f32; 4];

pub const IDENTITY: Coefs = [0.0; 4];

#[derive(Debug, Clone, PartialEq)]
pub struct Set {
    pub gain: f32,
    pub sections: Vec<Coefs>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixPair {
    /// First input (aux_mix: usb; centre_mix: centre; surround_mix: surround).
    pub a: [f32; 2],
    /// Second input (aux_mix: aux; centre_mix: direct; surround_mix: third).
    pub b: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BassSum {
    pub direct: [f32; 2],
    pub array: [f32; 2],
    pub surround: [f32; 2],
    pub lfe: f32,
}

/// Where a base comes from (informative, no identifier of the user or the speaker).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// `"stock-backup"` for a conversion, else free text.
    pub kind: String,
    pub firmware: Option<String>,
    pub backup_crc32: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Base {
    pub name: String,
    pub source: Option<Source>,
    /// `a` = USB, `b` = AUX.
    pub aux_mix: MixPair,
    /// L, C, R, LFE, Ls, Rs.
    pub input_gain: [f32; 6],
    /// `a` = centre, `b` = direct.
    pub centre_mix: MixPair,
    /// `a` = surround, `b` = third.
    pub surround_mix: MixPair,
    pub bass_sum: BassSum,
    /// Indexed like [`SETS`] ([`SetId::index`]).
    pub sets: Vec<Set>,
    /// In [`DELAYS`] order, samples at 48 kHz.
    pub delays: [u8; DELAYS.len()],
}

impl Base {
    pub fn set(&self, id: SetId) -> &Set {
        &self.sets[id.index()]
    }

    pub fn set_mut(&mut self, id: SetId) -> &mut Set {
        &mut self.sets[id.index()]
    }

    /// The 25 scalar floats in payload order (spec 5.3).
    pub fn scalars(&self) -> Vec<f32> {
        let pair = |m: &MixPair| [m.a[0], m.b[0], m.a[1], m.b[1]];
        let b = &self.bass_sum;
        let mut v = Vec::with_capacity(super::topology::SCALARS);
        v.extend(pair(&self.aux_mix));
        v.extend(self.input_gain);
        v.extend(pair(&self.centre_mix));
        v.extend(pair(&self.surround_mix));
        v.extend([b.direct[0], b.direct[1], b.array[0], b.array[1], b.surround[0], b.surround[1], b.lfe]);
        v
    }

    pub fn set_scalars(&mut self, v: &[f32]) {
        let pair = |s: &[f32]| MixPair { a: [s[0], s[2]], b: [s[1], s[3]] };
        self.aux_mix = pair(&v[0..4]);
        self.input_gain.copy_from_slice(&v[4..10]);
        self.centre_mix = pair(&v[10..14]);
        self.surround_mix = pair(&v[14..18]);
        self.bass_sum = BassSum { direct: [v[18], v[19]], array: [v[20], v[21]], surround: [v[22], v[23]], lfe: v[24] };
    }

    /// All floats in payload order: scalars, then each set (gain, then b1 b2 a1 a2 per section).
    pub fn floats(&self) -> Vec<f32> {
        let mut v = self.scalars();
        for s in &self.sets {
            v.push(s.gain);
            for c in &s.sections {
                v.extend(c);
            }
        }
        v
    }

    /// A base of the right shape with unity gains, identity sections and no delay (a starting point, not
    /// the neutral program of the firmware).
    pub fn blank(name: &str) -> Self {
        let one = MixPair { a: [1.0; 2], b: [0.0; 2] };
        Self {
            name: name.to_string(),
            source: None,
            aux_mix: MixPair { a: [1.0; 2], b: [1.0; 2] },
            input_gain: [1.0; 6],
            centre_mix: one,
            surround_mix: one,
            bass_sum: BassSum { direct: [0.0; 2], array: [0.0; 2], surround: [0.0; 2], lfe: 1.0 },
            sets: SETS.iter().map(|d| Set { gain: 1.0, sections: vec![IDENTITY; d.nsec] }).collect(),
            delays: [0; DELAYS.len()],
        }
    }
}
