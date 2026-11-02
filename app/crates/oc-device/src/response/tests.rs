//! Validation against `re/dsp/stock_sim.py` (fixture written by `re/dsp/base_response_ref.py`). The tests that
//! need the stock tables read the developer's own dump (`<project>/u7001_dsp.bin`, else the backups folder)
//! and are skipped without one: no Bose byte is in the repository.

use serde::Deserialize;

use super::complex::Spectrum;
use super::conditions::{loudness_boost_db, Conditions, SoundBase};
use super::stock::{Steady, StockGraph};
use super::ResponseError;

const FIXTURE: &str = include_str!("fixtures/stock_sim_ref.json");
/// Magnitude tolerance (dB) where the reference is within [`WINDOW`] dB of the output's peak.
const TOL: f64 = 0.1;
const WINDOW: f64 = 60.0;

#[derive(Deserialize)]
struct Fixture {
    freqs: Vec<f64>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    kind: String,
    #[serde(default)]
    direct: f64,
    #[serde(default)]
    array: f64,
    loud_band: f64,
    sources: std::collections::BTreeMap<String, Vec<Vec<f64>>>,
}

fn graph() -> Option<StockGraph> {
    let local = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../u7001_dsp.bin");
    if let Ok(b) = std::fs::read(local) {
        return Some(StockGraph::from_stock_dump(&b).expect("stock dump"));
    }
    match StockGraph::from_backups(&oc_install::backup::default_root()) {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("skipped: no stock DSP dump ({e})");
            None
        }
    }
}

fn db(h: &Spectrum) -> Vec<f64> {
    h.iter().map(|v| 10.0 * v.norm_sqr().max(1e-24).log10()).collect()
}

fn worst(g: &StockGraph, fx: &Fixture) -> (f64, usize, f64) {
    let w: Vec<f64> = fx.freqs.iter().map(|f| 2.0 * std::f64::consts::PI * f / 48_000.0).collect();
    let (mut worst, mut n, mut all): (f64, usize, f64) = (0.0, 0, 0.0);
    for c in &fx.cases {
        for (src, outs) in &c.sources {
            let src: usize = src.parse().unwrap();
            let got = match c.kind.as_str() {
                "bose" => g.responses(src, &Steady { loud_band: c.loud_band, direct: c.direct, array: c.array }, &w).unwrap(),
                _ => g.neutral_responses(src, c.loud_band, &w).unwrap(),
            };
            for (o, reference) in outs.iter().enumerate() {
                let peak = reference.iter().cloned().fold(f64::MIN, f64::max);
                if peak < -200.0 {
                    assert!(db(&got[o]).iter().all(|v| *v < -200.0), "{} src {src} out {o}: should be silent", c.kind);
                    continue;
                }
                for (i, (r, v)) in reference.iter().zip(db(&got[o])).enumerate() {
                    let e = (v - r).abs();
                    all = all.max(e);
                    if *r > peak - WINDOW {
                        n += 1;
                        assert!(e <= TOL, "{} src {src} out {o} at {} Hz: {v:.3} dB vs stock_sim {r:.3} dB", c.kind, fx.freqs[i]);
                        worst = worst.max(e);
                    }
                }
            }
        }
    }
    (worst, n, all)
}

#[test]
fn matches_stock_sim_within_0_1_db() {
    let Some(g) = graph() else { return };
    let fx: Fixture = serde_json::from_str(FIXTURE).unwrap();
    let (w, n, all) = worst(&g, &fx);
    assert!(n > 300, "only {n} points compared");
    eprintln!("vs stock_sim: worst {w:.2e} dB over {n} points within {WINDOW} dB of the peak (whole band {all:.2e} dB)");
}

#[test]
fn usb_level_is_the_stock_input_margin() {
    let Some(g) = graph() else { return };
    assert!((g.usb_level().unwrap() - 0.340_408).abs() < 1e-6);
}

#[test]
fn curves_are_finite_and_distinct_per_base() {
    let Some(g) = graph() else { return };
    let f: Vec<f64> = (0..50).map(|i| 20.0 * 1000f64.powf(f64::from(i) / 49.0)).collect();
    let bose = g.base_response(&Conditions::reference(SoundBase::Bose, true), &f).unwrap();
    let neutral = g.base_response(&Conditions::reference(SoundBase::Neutral, true), &f).unwrap();
    for r in [&bose, &neutral] {
        assert!(r.main.iter().chain(r.outputs.iter().flatten()).all(|v| v.is_finite()));
    }
    // Neutral: unity in the satellite pass band (1 kHz, loudness only acts around 50 Hz), stock high-pass below.
    let k1 = f.iter().position(|x| *x > 1000.0).unwrap();
    assert!(neutral.main[k1].abs() < 0.2, "{}", neutral.main[k1]);
    assert!(neutral.main[0] < -20.0);
    assert_ne!(bose.main, neutral.main);
}

#[test]
fn loudness_law() {
    // L = volume + 91 (detector floor -20 dB)
    assert!((loudness_boost_db(-20.0) - 5.2).abs() < 1e-9);
    assert!((loudness_boost_db(0.0) - 0.6).abs() < 1e-9);
    assert_eq!(loudness_boost_db(-80.0), 19.0);
    assert_eq!(Conditions::reference(SoundBase::Bose, false).loud_band(), 0.0);
    assert!(Conditions { base: SoundBase::Bose, loudness: true, volume_db: 10.0 }.loud_band() == 0.0);
}

#[test]
fn refuses_what_is_not_the_stock_image() {
    assert!(matches!(StockGraph::from_stock_dump(&[0xFF; 65536]), Err(ResponseError::Image(_))));
    assert!(matches!(StockGraph::from_stock_dump(&[1, 2, 3]), Err(ResponseError::Image(_))));
}
