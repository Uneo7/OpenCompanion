//! RBJ leading factors are folded into the set gain.

use super::schema::*;
use crate::base::coefs::{from_rbj, from_zpk, RbjKind, Roots};
use crate::base::model::{Base, BassSum, MixPair, Set, Source, IDENTITY};
use crate::base::topology::{SetId, NAME, SAMPLE_RATE};
use crate::base::{validate, BaseError};

fn bad<T>(s: String) -> Result<T, BaseError> {
    Err(BaseError::Json(s))
}

fn f(v: [f64; 2]) -> [f32; 2] {
    v.map(|x| x as f32)
}

fn roots(r: &RootsJson, what: &str) -> Result<Roots, BaseError> {
    match (r.f_hz, r.r, r.real) {
        (Some(f_hz), Some(r), None) => Ok(Roots::Pair { f_hz, r }),
        (None, None, Some(x)) => Ok(Roots::Real(x)),
        _ => bad(format!("{what}: give either f_hz and r, or real")),
    }
}

fn rbj_forms(s: &SectionObj) -> [(RbjKind, &Option<RbjJson>); 8] {
    [
        (RbjKind::Peak, &s.peak),
        (RbjKind::LowShelf, &s.low_shelf),
        (RbjKind::HighShelf, &s.high_shelf),
        (RbjKind::Lowpass, &s.lowpass),
        (RbjKind::Highpass, &s.highpass),
        (RbjKind::Bandpass, &s.bandpass),
        (RbjKind::Notch, &s.notch),
        (RbjKind::Allpass, &s.allpass),
    ]
}

/// (factor for the set gain, coefficients).
fn section(s: &SectionJson, at: &str) -> Result<(f64, [f32; 4]), BaseError> {
    let o = match s {
        SectionJson::Identity(_) => return Ok((1.0, IDENTITY)),
        SectionJson::Form(o) => o,
    };
    let rbj: Vec<_> = rbj_forms(o).into_iter().filter_map(|(k, v)| v.as_ref().map(|v| (k, v))).collect();
    let count = usize::from(o.raw.is_some()) + usize::from(o.zpk.is_some()) + rbj.len();
    if count != 1 {
        return bad(format!("{at}: exactly one section form expected, found {count}"));
    }
    if let Some(r) = o.raw {
        return Ok((1.0, r.map(|x| x as f32)));
    }
    if let Some(z) = &o.zpk {
        return Ok((1.0, from_zpk(roots(&z.zeros, at)?, roots(&z.poles, at)?)));
    }
    let (kind, p) = rbj[0];
    if kind.has_gain() != p.gain_db.is_some() {
        return bad(format!("{at}: {} {} gain_db", kind.key(), if kind.has_gain() { "needs" } else { "takes no" }));
    }
    from_rbj(kind, p.f_hz, p.q, p.gain_db.unwrap_or(0.0)).map_err(|e| BaseError::Json(format!("{at}: {e}")))
}

fn set(s: &SetJson, id: SetId) -> Result<Set, BaseError> {
    let d = id.def();
    let at = if d.key.is_empty() { d.block.to_string() } else { format!("{}.{}", d.block, d.key) };
    let mut g = match (s.gain, s.gain_db) {
        (Some(g), None) if !s.invert => g,
        (None, Some(db)) => 10f64.powf(db / 20.0) * if s.invert { -1.0 } else { 1.0 },
        _ => return bad(format!("{at}: give exactly one of gain, or gain_db (with invert)")),
    };
    let mut sections = Vec::with_capacity(s.sections.len());
    for (k, sec) in s.sections.iter().enumerate() {
        let (factor, c) = section(sec, &format!("{at} section {k}"))?;
        g *= factor;
        sections.push(c);
    }
    Ok(Set { gain: g as f32, sections })
}

pub fn from_json(j: &FileJson) -> Result<Base, BaseError> {
    if j.format != FORMAT || j.version != VERSION {
        return bad(format!("not an OpenCompanion base v{VERSION} ({} v{})", j.format, j.version));
    }
    if j.topology != NAME || j.sample_rate != SAMPLE_RATE {
        return bad(format!("topology {} at {} Hz, expected {NAME} at {SAMPLE_RATE} Hz", j.topology, j.sample_rate));
    }
    let mut b = Base::blank(&j.name);
    b.source = j.source.as_ref().map(|s| Source {
        kind: s.kind.clone(),
        firmware: s.firmware.clone(),
        backup_crc32: s.backup_crc32.as_deref().and_then(|h| u32::from_str_radix(h.trim_start_matches("0x"), 16).ok()),
    });
    b.aux_mix = MixPair { a: f(j.aux_mix.usb), b: f(j.aux_mix.aux) };
    b.input_gain = j.input_gain.map(|x| x as f32);
    b.centre_mix = MixPair { a: f(j.centre_mix.centre), b: f(j.centre_mix.direct) };
    b.surround_mix = MixPair { a: f(j.surround_mix.surround), b: f(j.surround_mix.third) };
    let bs = &j.bass_sum;
    b.bass_sum = BassSum { direct: f(bs.direct), array: f(bs.array), surround: f(bs.surround), lfe: bs.lfe as f32 };
    let (de, sn, fnet) = (&j.driver_eq, &j.surround_net, &j.front_net);
    let all: [(SetId, &SetJson); 16] = [
        (SetId::EqDirect, &de.direct),
        (SetId::EqArray, &de.array),
        (SetId::EqSurround, &de.surround),
        (SetId::EqBass, &de.bass),
        (SetId::SurPre, &sn.pre),
        (SetId::SurStage1, &sn.stage1),
        (SetId::SurStage2, &sn.stage2),
        (SetId::SurStage3, &sn.stage3),
        (SetId::FrontDirect, &fnet.direct),
        (SetId::FrontArrayA, &fnet.array_a),
        (SetId::FrontArrayX, &fnet.array_x),
        (SetId::FrontStageB, &fnet.stage_b),
        (SetId::FrontStageBX, &fnet.stage_b_x),
        (SetId::FrontStageC, &fnet.stage_c),
        (SetId::FrontStageCX, &fnet.stage_c_x),
        (SetId::BassPhase, &j.bass_phase),
    ];
    for (id, s) in all {
        *b.set_mut(id) = set(s, id)?;
    }
    let d = [sn.delay_in, sn.cross1, sn.cross2, fnet.delay];
    for (k, v) in d.iter().flatten().enumerate() {
        b.delays[k] = *v;
    }
    validate::check(&b)?;
    Ok(b)
}
