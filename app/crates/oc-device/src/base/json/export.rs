//! [`Base`] → JSON. `raw` sections carry each float32 as its shortest decimal (reads back to the same
//! float32); `zpk` writes pole / zero forms (readable, one rounding on the way back).

use serde_json::{json, Value};

use super::schema::*;
use crate::base::coefs::{zpk, Roots};
use crate::base::model::{Base, Coefs, Set};
use crate::base::topology::{SetDef, SetId, NAME, SAMPLE_RATE};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Form {
    #[default]
    Raw,
    Zpk,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub form: Form,
    pub annotate: bool,
}

/// Shortest decimal of a float32, as an f64 that serde_json prints with the same digits.
pub fn num(x: f32) -> f64 {
    format!("{x}").parse().expect("a float32 prints as a number")
}

fn pair(v: [f32; 2]) -> [f64; 2] {
    v.map(num)
}

fn roots_json(r: Roots) -> RootsJson {
    let round = |x: f64| (x * 1e9).round() / 1e9;
    match r {
        Roots::Pair { f_hz, r } => RootsJson { f_hz: Some(round(f_hz)), r: Some(round(r)), real: None },
        Roots::Real(x) => RootsJson { real: Some(x.map(round)), ..Default::default() },
    }
}

fn roots_about(r: Roots) -> Value {
    match r {
        Roots::Pair { f_hz, r } => json!({ "f_hz": (f_hz * 10.0).round() / 10.0, "r": (r * 1e5).round() / 1e5 }),
        Roots::Real([a, b]) => json!({ "real": [(a * 1e5).round() / 1e5, (b * 1e5).round() / 1e5] }),
    }
}

fn section(c: &Coefs, role: Option<&str>, o: Options) -> SectionJson {
    let (z, p) = zpk(c);
    let mut s = SectionObj::default();
    match o.form {
        Form::Raw => s.raw = Some(c.map(num)),
        Form::Zpk => s.zpk = Some(ZpkJson { zeros: roots_json(z), poles: roots_json(p) }),
    }
    if o.annotate {
        let mut a = json!({ "zeros": roots_about(z), "poles": roots_about(p) });
        if let Some(r) = role {
            a["role"] = json!(r);
        }
        s.about = Some(a);
    }
    SectionJson::Form(Box::new(s))
}

fn set(b: &Base, id: SetId, o: Options) -> SetJson {
    let d: &SetDef = id.def();
    let s: &Set = b.set(id);
    let sections = s
        .sections
        .iter()
        .enumerate()
        .map(|(k, c)| if *c == crate::base::model::IDENTITY { SectionJson::Identity(IdentityTag::Identity) } else { section(c, d.roles.get(k).map(|r| r.name()), o) })
        .collect();
    SetJson { gain: Some(num(s.gain)), sections, ..Default::default() }
}

pub fn to_json(b: &Base, o: Options) -> FileJson {
    let source = b.source.as_ref().map(|s| SourceJson {
        kind: s.kind.clone(),
        firmware: s.firmware.clone(),
        backup_crc32: s.backup_crc32.map(|c| format!("0x{c:08X}")),
    });
    let about = o.annotate.then(|| {
        json!("Parametric base, docs/developer/base-format.md. 'raw' sections are exact float32 values; every \
               'about' field is an annotation (roles are an interpretation of the pole / zero positions) and is \
               ignored on import.")
    });
    let bs = &b.bass_sum;
    let dl = &b.delays;
    FileJson {
        format: FORMAT.into(),
        version: VERSION,
        topology: NAME.into(),
        name: b.name.clone(),
        source,
        sample_rate: SAMPLE_RATE,
        about,
        aux_mix: AuxMixJson { usb: pair(b.aux_mix.a), aux: pair(b.aux_mix.b), about: None },
        input_gain: b.input_gain.map(num),
        centre_mix: CentreMixJson { direct: pair(b.centre_mix.b), centre: pair(b.centre_mix.a), about: None },
        surround_mix: SurroundMixJson { surround: pair(b.surround_mix.a), third: pair(b.surround_mix.b), about: None },
        bass_sum: BassSumJson { direct: pair(bs.direct), array: pair(bs.array), surround: pair(bs.surround), lfe: num(bs.lfe), about: None },
        driver_eq: DriverEqJson {
            direct: set(b, SetId::EqDirect, o),
            array: set(b, SetId::EqArray, o),
            surround: set(b, SetId::EqSurround, o),
            bass: set(b, SetId::EqBass, o),
            about: None,
        },
        bass_phase: set(b, SetId::BassPhase, o),
        surround_net: SurroundNetJson {
            delay_in: [dl[0], dl[1]],
            pre: set(b, SetId::SurPre, o),
            stage1: set(b, SetId::SurStage1, o),
            cross1: [dl[2], dl[3]],
            stage2: set(b, SetId::SurStage2, o),
            cross2: [dl[4], dl[5]],
            stage3: set(b, SetId::SurStage3, o),
            about: None,
        },
        front_net: FrontNetJson {
            direct: set(b, SetId::FrontDirect, o),
            array_a: set(b, SetId::FrontArrayA, o),
            array_x: set(b, SetId::FrontArrayX, o),
            stage_b: set(b, SetId::FrontStageB, o),
            stage_b_x: set(b, SetId::FrontStageBX, o),
            delay: [dl[6], dl[7]],
            stage_c: set(b, SetId::FrontStageC, o),
            stage_c_x: set(b, SetId::FrontStageCX, o),
            about: None,
        },
    }
}
