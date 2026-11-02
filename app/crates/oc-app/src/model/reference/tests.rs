//! `OC_REGEN_REFERENCE=1 cargo test -p oc-app reference` rewrites `bose_curves.bin` from a stock U7001 backup
//! (`<project>/u7001_dsp.bin`, else the app's backups folder). Without the variable, the embedded file is
//! checked against the same computation when a backup is present.

use oc_device::response::{Conditions, SoundBase, StockGraph};

use super::{curves, encode, parse, Curves, DATA};
use crate::model::biquad::{FREQS, POINTS};
use crate::model::eq::Target;

const FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/model/reference/bose_curves.bin");

fn stock_graph() -> Option<StockGraph> {
    let local = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../u7001_dsp.bin");
    match std::fs::read(local) {
        Ok(b) => Some(StockGraph::from_stock_dump(&b).expect("stock dump")),
        Err(_) => StockGraph::from_backups(&oc_device::install::default_backups()).ok(),
    }
}

fn compute(g: &StockGraph) -> [Curves; 2] {
    let pts = |v: &[f64]| -> [f64; POINTS] { std::array::from_fn(|i| v[i]) };
    [false, true].map(|loud| {
        let r = g.base_response(&Conditions::reference(SoundBase::Bose, loud), &FREQS[..]).expect("Bose response");
        Curves { main: pts(&r.main), outputs: r.outputs.map(|o| pts(&o)) }
    })
}

#[test]
fn embedded_curves_match_the_computation() {
    let Some(g) = stock_graph() else {
        eprintln!("skipped: no stock DSP backup to check bose_curves.bin against");
        return;
    };
    let fresh = compute(&g);
    if std::env::var_os("OC_REGEN_REFERENCE").is_some() {
        std::fs::write(FILE, encode(&fresh)).expect("writing bose_curves.bin");
        eprintln!("bose_curves.bin regenerated (rebuild to embed it)");
        return;
    }
    let emb = parse(DATA).expect("embedded file parses");
    for (e, f) in emb.iter().zip(&fresh) {
        for (a, b) in std::iter::once(&e.main).chain(&e.outputs).zip(std::iter::once(&f.main).chain(&f.outputs)) {
            for (x, y) in a.iter().zip(b) {
                assert!((x - y).abs() < 1e-3, "embedded {x} dB vs computed {y} dB: regenerate bose_curves.bin");
            }
        }
    }
}

#[test]
fn embedded_curves_are_complete_and_plausible() {
    for loud in [false, true] {
        let c = curves(loud);
        assert!(c.main.iter().chain(c.outputs.iter().flatten()).all(|v| v.is_finite()));
        // satellites: crossover stop band at 20 Hz; bass: stop band at 20 kHz, pass band around 50 Hz
        assert!(c.of(Target::InnerLeft)[0] < -40.0);
        assert!(c.of(Target::Sub)[POINTS - 1] < -60.0);
        let k50 = FREQS.iter().position(|f| *f >= 50.0).unwrap();
        assert!(c.of(Target::Sub)[k50] > 0.0);
    }
    // the loudness lifts the bass at the reference volume
    let k50 = FREQS.iter().position(|f| *f >= 50.0).unwrap();
    assert!(curves(true).of(Target::Sub)[k50] > curves(false).of(Target::Sub)[k50] + 3.0);
}

#[test]
fn file_round_trips_and_refuses_other_layouts() {
    let c = parse(DATA).unwrap();
    assert_eq!(encode(&c), DATA);
    assert!(parse(&DATA[..DATA.len() - 4]).is_none());
    let mut bad = DATA.to_vec();
    bad[6] ^= 1; // points
    assert!(parse(&bad).is_none());
}

#[test]
fn targets_map_to_their_output() {
    let c = Curves { main: [0.0; POINTS], outputs: std::array::from_fn(|k| [k as f64 + 1.0; POINTS]) };
    assert_eq!(c.of(Target::Global)[0], 0.0);
    assert_eq!(c.of(Target::InnerLeft)[0], 1.0);
    assert_eq!(c.of(Target::OuterRight)[0], 4.0);
    assert_eq!(c.of(Target::Sub)[0], 5.0);
}
