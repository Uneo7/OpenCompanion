//! Golden test: the base converted from the developer's stock dump equals, bit for bit, the stock values the
//! firmware runs today (`firmware/dsp/src/bose/bose_graph_data.c` `bg_f` / `bg_bq` / `bg_dl`, and the bass
//! all-pass of `bose_graph.c`). The tests that need the dump read `<project>/u7001_dsp.bin`, else
//! `<project>/backups/2026-10-31/u7001_dsp.bin`, else the backups folder, and are skipped without one: no
//! Bose byte is in the repository.

use super::coefs::{from_rbj, RbjKind};
use super::json::{self, Form, Options};
use super::model::Base;
use super::payload;
use super::topology::{payload_len, SetId, SETS};
use super::{convert, validate, BaseError};

const FW: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../firmware/dsp/src/bose/");

fn stock() -> Option<Base> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../");
    for p in ["u7001_dsp.bin", "backups/2026-10-31/u7001_dsp.bin"] {
        if let Ok(b) = std::fs::read(format!("{root}{p}")) {
            return Some(convert::from_stock_dump(&b).expect("stock dump converts"));
        }
    }
    let r = crate::response::StockGraph::from_backups(&oc_install::backup::default_root());
    match r {
        Ok(g) => Some(convert::from_stock(&g, None).expect("stock backup converts")),
        Err(e) => {
            eprintln!("skipped: no stock DSP dump ({e})");
            None
        }
    }
}

fn c_array<'a>(src: &'a str, decl: &str) -> &'a str {
    let start = src.find(decl).expect("array declaration") + decl.len();
    let end = start + src[start..].find("};").expect("end of array");
    &src[start..end]
}

fn c_floats(s: &str) -> Vec<f32> {
    s.split([',', '{', '}', ' ', '\n', '\r'])
        .map(|t| t.trim().trim_end_matches('f'))
        .filter(|t| !t.is_empty())
        .map(|t| t.parse::<f32>().unwrap_or_else(|_| panic!("float {t}")))
        .collect()
}

fn c_rows(s: &str) -> Vec<Vec<u32>> {
    s.lines()
        .filter(|l| l.trim_start().starts_with('{'))
        .map(|l| {
            l.split(|c: char| !c.is_ascii_digit()).filter(|t| !t.is_empty()).map(|t| t.parse().unwrap()).collect()
        })
        .collect()
}

fn firmware_reference() -> (Vec<f32>, Vec<u8>) {
    let data = std::fs::read_to_string(format!("{FW}bose_graph_data.c")).expect("bose_graph_data.c");
    let graph = std::fs::read_to_string(format!("{FW}bose_graph.c")).expect("bose_graph.c");
    let f = c_floats(c_array(&data, "bg_f[BG_NF] = {"));
    let bq = c_rows(c_array(&data, "bg_bq[BG_NBQ] = {"));
    let dl = c_rows(c_array(&data, "bg_dl[BG_NDL] = {"));
    // Scalars: aux mixers, realign gain, centre and surround mixers, bass sum (stock storage order).
    let mut v: Vec<f32> = [(33, 4), (59, 6), (88, 8), (116, 7)].iter().flat_map(|&(a, n)| f[a..a + n].to_vec()).collect();
    // Sets: driver EQ (bg_bq[5], 4 sets), then bg_bq[6..=16] (one set each).
    let mut set = |b: usize, s: usize| {
        let (coef, nsec) = (bq[b][0] as usize, bq[b][2] as usize);
        let base = coef + s * (1 + 4 * nsec);
        v.extend_from_slice(&f[base..base + 1 + 4 * nsec]);
    };
    (0..4).for_each(|s| set(5, s));
    (6..=16).for_each(|b| set(b, 0));
    // Bass all-pass: gain and AP_SEC of bose_graph.c.
    // DSP 0.2.4 (48 kHz only): `biquad_init(&ap, …)`, before: `rate_biquad(&ap, …)` (same arguments).
    let g = graph
        .split("biquad_init(&ap, ")
        .nth(1)
        .or_else(|| graph.split("rate_biquad(&ap, ").nth(1))
        .expect("biquad_init(&ap")
        .split(',')
        .next()
        .unwrap();
    v.extend(c_floats(g));
    v.extend(c_floats(c_array(&graph, "AP_SEC = {")));
    (v, dl.iter().map(|r| r[1] as u8).collect())
}

#[test]
fn golden_stock_base_equals_bg_f() {
    let Some(b) = stock() else { return };
    let (want, delays) = firmware_reference();
    let got = b.floats();
    assert_eq!(got.len(), want.len());
    for (k, (g, w)) in got.iter().zip(&want).enumerate() {
        assert_eq!(g.to_bits(), w.to_bits(), "payload float {k}: {g} vs firmware {w}");
    }
    assert_eq!(b.delays.to_vec(), delays);
}

#[test]
fn stock_base_passes_the_hard_rules_and_fits() {
    let Some(b) = stock() else { return };
    validate::check(&b).unwrap();
    let bytes = payload::encode(&b, 7).unwrap();
    assert_eq!(bytes.len(), payload::HDR_LEN + payload_len());
    assert_eq!(payload_len(), 1132);
    assert!(payload_len() <= payload::CAPACITY);
    let (seq, back) = payload::decode(&bytes, &b.name).unwrap();
    assert_eq!(seq, 7);
    assert_eq!(back.floats().iter().map(|x| x.to_bits()).collect::<Vec<_>>(), b.floats().iter().map(|x| x.to_bits()).collect::<Vec<_>>());
    let mut bad = bytes.clone();
    bad[40] ^= 1;
    assert!(matches!(payload::decode(&bad, "x"), Err(BaseError::Payload(_))));
}

#[test]
fn json_raw_round_trip_is_bit_exact() {
    let Some(b) = stock() else { return };
    let text = json::export(&b, Options { form: Form::Raw, annotate: true });
    let back = json::import(&text).unwrap();
    let bits = |b: &Base| b.floats().iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&back), bits(&b));
    assert_eq!(back.delays, b.delays);
    assert_eq!(back.source, b.source);
}

#[test]
fn json_zpk_round_trip_is_within_rounding() {
    let Some(b) = stock() else { return };
    let back = json::import(&json::export(&b, Options { form: Form::Zpk, annotate: false })).unwrap();
    for (d, (s, t)) in SETS.iter().zip(b.sets.iter().zip(&back.sets)) {
        for (k, (c, e)) in s.sections.iter().zip(&t.sections).enumerate() {
            for i in 0..4 {
                let tol = 4e-7 * c[i].abs().max(1.0);
                assert!((c[i] - e[i]).abs() <= tol, "{}.{} s{k}[{i}]: {} vs {}", d.block, d.key, c[i], e[i]);
            }
        }
    }
}

#[test]
fn json_refuses_unknown_keys_and_bad_counts() {
    let mut b = Base::blank("t");
    let text = json::export(&b, Options::default());
    assert!(json::import(&text).is_ok());
    let typo = text.replacen("\"input_gain\"", "\"input_gains\"", 1);
    assert!(matches!(json::import(&typo), Err(BaseError::Json(_))));
    b.set_mut(SetId::EqBass).sections.pop();
    assert!(matches!(json::import(&json::export(&b, Options::default())), Err(BaseError::Invalid(_))));
}

#[test]
fn rbj_peak_has_its_gain_at_its_frequency() {
    let (g, c) = from_rbj(RbjKind::Peak, 1000.0, 1.0, 6.0).unwrap();
    let w = 2.0 * std::f64::consts::PI * 1000.0 / 48_000.0;
    let (z1, z2) = ((w.cos(), -w.sin()), ((2.0 * w).cos(), -(2.0 * w).sin()));
    let c = c.map(f64::from);
    let num = (1.0 + c[0] * z1.0 + c[1] * z2.0, c[0] * z1.1 + c[1] * z2.1);
    let den = (1.0 - c[2] * z1.0 - c[3] * z2.0, -c[2] * z1.1 - c[3] * z2.1);
    let mag = g * (num.0.hypot(num.1) / den.0.hypot(den.1));
    assert!((20.0 * mag.log10() - 6.0).abs() < 1e-4);
}

#[test]
fn unstable_sections_are_refused() {
    assert!(validate::section_ok(&[0.0, 0.0, 1.9, -0.95]).is_ok());
    assert!(validate::section_ok(&[0.0, 0.0, 2.0, -1.0]).is_err());
    assert!(validate::section_ok(&[0.0, 0.0, 0.5, 0.6]).is_err());
    assert!(validate::section_ok(&[9.0, 0.0, 0.0, 0.0]).is_err());
    let mut b = Base::blank("t");
    b.set_mut(SetId::EqDirect).sections[0] = [0.0, 0.0, 2.0, -1.0];
    assert!(matches!(validate::check(&b), Err(BaseError::Invalid(_))));
}
