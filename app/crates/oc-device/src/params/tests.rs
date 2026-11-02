//! Checks the Rust layout against `params.h`, `params.c` and `profiles/bose_stock.json`.

use super::layout::{band_off, band_offset, limits, off, target_range, BAND_SIZE};
use super::*;

const PARAMS_H: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../firmware/dsp/include/params.h"));
const PARAMS_C: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../firmware/dsp/src/control/params.c"));
const BOSE_JSON: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../profiles/bose_stock.json"));

fn c_layout_table() -> Vec<(usize, Option<usize>, String)> {
    let mut rows = Vec::new();
    for line in PARAMS_H.lines() {
        let Some(rest) = line.trim_start().strip_prefix('*') else { continue };
        let tok: Vec<&str> = rest.split_whitespace().collect();
        let Some(o) = tok.first().and_then(|t| t.parse::<usize>().ok()) else { continue };
        match tok.get(1).map(|t| t.parse::<usize>()) {
            Some(Ok(size)) => rows.push((o, Some(size), tok.get(2).unwrap_or(&"").to_string())),
            _ => rows.push((o, None, tok.get(1).unwrap_or(&"").to_string())),
        }
    }
    rows
}

fn c_define(name: &str) -> i64 {
    let line = PARAMS_H
        .lines()
        .find(|l| l.split_whitespace().take(2).eq(["#define", name]))
        .unwrap_or_else(|| panic!("#define {name} not found in params.h"));
    let v = line.split_whitespace().nth(2).unwrap();
    match v.strip_prefix("0x") {
        Some(h) => i64::from_str_radix(h, 16).unwrap(),
        None => v.parse().unwrap(),
    }
}

fn c_enum(first: &str) -> Vec<String> {
    let start = PARAMS_H.find(&format!("enum {{ {first}")).expect("enum not found");
    let body = &PARAMS_H[start + 6..];
    let body = &body[..body.find('}').unwrap()];
    body.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

#[test]
fn layout_table_matches_params_h() {
    let expected: [(usize, usize, &str); 12] = [
        (off::VERSION, 1, "version"),
        (off::FLAGS, 1, "flags"),
        (off::COLOR, 1, "color"),
        (off::RESERVED0, 1, "reserved"),
        (off::NAME, NAME_LEN, "name"),
        (off::SUB_LEVEL, 2, "sub_level_cdb"),
        (off::CROSSOVER, 2, "crossover_hz"),
        (off::AUX_GAIN, 2, "aux_gain_cdb"),
        (off::TRIMS, 2 * EQ_OUTPUTS, "trim_cdb[5]"),
        (off::BASS, 2, "reserved"),
        (off::RESERVED1, 2, "reserved"),
        (off::BANDS, EQ_TARGETS * EQ_MAX_BANDS * BAND_SIZE, "band[6][12]"),
    ];
    let table = c_layout_table();
    let fields: Vec<_> = table.iter().filter(|r| r.1.is_some()).map(|(o, s, n)| (*o, s.unwrap(), n.as_str())).collect();
    assert_eq!(fields, expected.to_vec());
    // Contiguous, and the "(size)" row closes the table.
    for w in expected.windows(2) {
        assert_eq!(w[0].0 + w[0].1, w[1].0, "gap or overlap after {}", w[0].2);
    }
    let size_row = table.iter().find(|r| r.2 == "(size)").expect("(size) row");
    assert_eq!(size_row.0, PROFILE_SIZE);
    let last = expected[expected.len() - 1];
    assert_eq!(last.0 + last.1, PROFILE_SIZE);
}

#[test]
fn defines_match_params_h() {
    assert_eq!(c_define("PROFILE_VERSION"), i64::from(PROFILE_VERSION));
    assert_eq!(c_define("PROFILE_SIZE"), PROFILE_SIZE as i64);
    assert_eq!(c_define("PROFILE_BANDS_OFF"), off::BANDS as i64);
    assert_eq!(c_define("EQ_MAX_BANDS"), EQ_MAX_BANDS as i64);
    assert_eq!(c_define("EQ_TARGETS"), EQ_TARGETS as i64);
    assert_eq!(c_define("EQ_OUTPUTS"), EQ_OUTPUTS as i64);
    assert_eq!(c_define("PROFILE_NAME_LEN"), NAME_LEN as i64);
    assert_eq!(c_define("PF_BYPASS"), i64::from(flags::BYPASS));
    assert_eq!(c_define("PF_LINK_LR"), i64::from(flags::LINK_LR));
    assert_eq!(c_define("PF_LOUDNESS"), i64::from(flags::LOUDNESS));
    assert_eq!(c_define("PF_VOL_STOCK"), i64::from(flags::VOL_STOCK));
    assert_eq!(c_define("PF_STOCK_GRAPH"), i64::from(flags::STOCK_GRAPH));
    assert_eq!(c_define("PF_ADVANCED"), i64::from(flags::ADVANCED));
    assert!(PARAMS_C.contains("PF_STOCK_GRAPH | PF_ADVANCED;"), "params_sanitize() keeps PF_ADVANCED");
    assert!(!PARAMS_H.contains("BOSE_EQ_SECTIONS"), "the Bose-base EQ cap is gone (protocol 5)");
    assert_eq!(c_define("BAND_DISABLED"), i64::from(BAND_DISABLED));
    // The static size checks of the header use the same values.
    assert!(PARAMS_H.contains("sizeof(eq_band_t) == 8"));
    assert!(PARAMS_H.contains("offsetof(profile_t, reserved36) == 36"));
    assert_eq!(BAND_SIZE, 8);
    assert_eq!(off::BASS, 36);
}

#[test]
fn enums_match_params_h() {
    let targets = c_enum("EQ_GLOBAL");
    assert_eq!(targets, ["EQ_GLOBAL", "EQ_INNER_L", "EQ_INNER_R", "EQ_OUTER_L", "EQ_OUTER_R", "EQ_SUB"]);
    assert_eq!(EqTarget::ALL.map(EqTarget::index), [0, 1, 2, 3, 4, 5]);
    let kinds = c_enum("BAND_OFF");
    let expected =
        ["BAND_OFF", "BAND_PEAK", "BAND_LOW_SHELF", "BAND_HIGH_SHELF", "BAND_HPF", "BAND_LPF", "BAND_NOTCH", "BAND_TYPE_COUNT"];
    assert_eq!(kinds, expected);
    for (i, k) in BandKind::ALL.iter().enumerate() {
        assert_eq!(usize::from(k.byte()), i);
    }
    assert_eq!(BandKind::from_byte(7), None, "BAND_TYPE_COUNT is not a type (7, 8: former raw pairs)");
}

#[test]
fn band_struct_matches_params_h() {
    // "off 0 type, 1 reserved, 2 freq_hz (...), 4 gain_cdb (...), 6 q_milli"
    for (o, name) in [(band_off::TYPE, "type"), (band_off::RESERVED, "reserved"), (band_off::FREQ, "freq_hz"), (band_off::GAIN, "gain_cdb"), (band_off::Q, "q_milli")] {
        assert!(PARAMS_H.contains(&format!("{o} {name}")), "eq_band_t comment: {o} {name}");
    }
    let s = BandSlot::new(BandKind::LowShelf, 0x1234, -2, 0xABCD);
    assert_eq!(s.to_bytes(), [2, 0, 0x34, 0x12, 0xFE, 0xFF, 0xCD, 0xAB]);
    assert_eq!(BandSlot::from_bytes(&s.to_bytes()), s);
    assert_eq!(band_offset(EqTarget::Global, 0), 40);
    assert_eq!(band_offset(EqTarget::InnerL, 0), 40 + 96);
    assert_eq!(band_offset(EqTarget::Sub, 11), PROFILE_SIZE - 8);
    assert_eq!(target_range(EqTarget::OuterR), 40 + 4 * 96..40 + 5 * 96);
}

#[test]
fn limits_match_params_c() {
    let has = |s: &str| assert!(PARAMS_C.contains(s), "params.c: {s}");
    has(&format!("clamp16(p->sub_level_cdb, {}, {})", limits::SUB_LEVEL_CDB.0, limits::SUB_LEVEL_CDB.1));
    has("p->crossover_hz = 0;");
    has("p->reserved36 = 0;");
    has(&format!("clamp16(p->aux_gain_cdb, {}, {})", limits::AUX_GAIN_CDB.0, limits::AUX_GAIN_CDB.1));
    has(&format!("clamp16(p->trim_cdb[i], {}, {})", limits::TRIM_CDB.0, limits::TRIM_CDB.1));
    has(&format!("clamp16(bd->freq_hz, {}, {})", limits::FREQ_HZ.0, limits::FREQ_HZ.1));
    has(&format!("clamp16(bd->gain_cdb, {}, {})", limits::GAIN_CDB.0, limits::GAIN_CDB.1));
    has(&format!("q_milli < {} ? {}", limits::Q_MILLI.0, limits::Q_MILLI.0));
    has(&format!("q_milli > {} ? {}", limits::Q_MILLI.1, limits::Q_MILLI.1));
    let freqs = DEFAULT_FREQS.map(|f| f.to_string()).join(", ");
    has(&format!("freqs[EQ_MAX_BANDS] = {{ {freqs} }}"));
}

#[test]
fn defaults_round_trip() {
    let p = Profile::defaults();
    assert_eq!(p.name(), "Flat");
    let b = p.to_bytes();
    assert_eq!(b.len(), PROFILE_SIZE);
    assert_eq!(b[off::VERSION], PROFILE_VERSION);
    assert_eq!(&b[off::NAME..off::NAME + 5], b"Flat\0");
    assert_eq!(Profile::from_bytes(&b).unwrap(), p);
    assert_eq!(p.clone().sanitized(), p);
    assert_eq!(Profile::from_bytes(&b[..10]), Err(LayoutError(10)));
}

#[test]
fn names_are_cut_on_char_boundaries() {
    let mut p = Profile::defaults();
    p.set_name("Films nuit très tard");
    assert_eq!(p.name(), "Films nuit très");
    p.set_name("éééééééé€");
    assert_eq!(p.name(), "éééééééé");
    assert_eq!(name_from_bytes(b"abc\0\xff"), "abc");
}

fn json_band(v: &serde_json::Value) -> BandSlot {
    let kind = match v["type"].as_str().unwrap() {
        "off" => BandKind::Off,
        "peak" => BandKind::Peak,
        other => panic!("unexpected band type {other}"),
    };
    BandSlot {
        kind: kind.byte(),
        reserved: v["reserved"].as_u64().unwrap() as u8,
        freq_hz: v["freq_hz"].as_u64().unwrap() as u16,
        gain_cdb: v["gain_cdb"].as_i64().unwrap() as i16,
        q_milli: v["q_milli"].as_u64().unwrap() as u16,
    }
}

#[test]
fn bose_profile_matches_the_generated_json() {
    let j: serde_json::Value = serde_json::from_str(BOSE_JSON).unwrap();
    assert_eq!(j["size"].as_u64(), Some(PROFILE_SIZE as u64));
    let mut p = Profile::defaults();
    p.version = j["version"].as_u64().unwrap() as u8;
    p.flags = j["flags"].as_u64().unwrap() as u8;
    p.color = j["color"].as_u64().unwrap() as u8;
    p.set_name(j["name"].as_str().unwrap());
    p.sub_level_cdb = j["sub_level_cdb"].as_i64().unwrap() as i16;
    p.crossover_hz = j["crossover_hz"].as_u64().unwrap() as u16;
    p.aux_gain_cdb = j["aux_gain_cdb"].as_i64().unwrap() as i16;
    for (i, t) in j["trim_cdb"].as_array().unwrap().iter().enumerate() {
        p.trim_cdb[i] = t.as_i64().unwrap() as i16;
    }
    for (t, list) in j["band"].as_array().unwrap().iter().enumerate() {
        for (b, v) in list.as_array().unwrap().iter().enumerate() {
            p.bands[t][b] = json_band(v);
        }
    }
    assert_eq!(p.flags, flags::LOUDNESS | flags::STOCK_GRAPH);
    assert!(p.bands.iter().flatten().all(|b| b.is_off()), "the built-in profile is the Bose base, flat");
    assert_eq!(p.clone().sanitized(), p);
    assert_eq!(Profile::from_bytes(&p.to_bytes()).unwrap(), p);
}

#[test]
fn bose_base_has_no_eq_cap() {
    // DSP profile protocol 5: the 4-lane EQ banks made the Bose-base CPU cap (BOSE_EQ_SECTIONS) unnecessary.
    let peak = BandSlot { kind: BandKind::Peak.byte(), reserved: 0, freq_hz: 1000, gain_cdb: 300, q_milli: 707 };
    let mut p = Profile::defaults();
    for t in 0..EQ_TARGETS {
        for b in 0..EQ_MAX_BANDS {
            p.bands[t][b] = peak;
        }
    }
    p.set_flag(flags::STOCK_GRAPH, true);
    assert_eq!(p.clone().sanitized().bands, p.bands);
}

#[test]
fn disabled_bands_keep_their_type() {
    let mut b = BandSlot::new(BandKind::LowShelf, 120, 450, 900);
    b.set_enabled(false);
    assert_eq!(b.kind, 0x82);
    assert!(b.is_disabled() && !b.is_enabled() && !b.is_off() && b.band_kind().is_none());
    assert_eq!(b.kept_kind(), Some(BandKind::LowShelf));
    let mut p = Profile::defaults();
    p.bands[0][4] = b;
    p.bands[0][5] = BandSlot { kind: BAND_DISABLED, ..BandSlot::off_at(100) }; // off | disabled: invalid
    p.bands[0][6] = BandSlot { kind: BAND_DISABLED | 7, ..BandSlot::off_at(100) }; // invalid (former raw type)
    p.bands[0][7] = BandSlot { kind: BAND_DISABLED | BandKind::Notch.byte(), freq_hz: 5, ..BandSlot::off_at(100) };
    let q = p.sanitized();
    assert_eq!(q.bands[0][4], b, "survives the firmware rules (OPW / reload)");
    assert!(q.bands[0][5].is_off() && q.bands[0][6].is_off());
    assert_eq!((q.bands[0][7].kind, q.bands[0][7].freq_hz), (0x86, 20), "disabled bands are clamped too");
    let mut on = q.bands[0][4];
    on.set_enabled(true);
    assert_eq!(on, BandSlot::new(BandKind::LowShelf, 120, 450, 900));
    let mut off = BandSlot::off_at(50);
    off.set_enabled(false);
    assert!(off.is_off(), "no effect on an empty slot");
}

#[test]
fn sanitize_clamps_like_the_firmware() {
    let mut p = Profile::defaults();
    p.flags = 0xFF;
    p.sub_level_cdb = 5000;
    p.crossover_hz = 20;
    p.trim_cdb[4] = 900;
    p.bass_milli = -3000;
    p.bands[0][0] = BandSlot { kind: 1, reserved: 7, freq_hz: 5, gain_cdb: -2000, q_milli: 50_000 };
    p.bands[1][3] = BandSlot { kind: 12, ..BandSlot::off_at(100) };
    p.bands[2][11] = BandSlot { kind: 7, ..BandSlot::off_at(100) }; // former BAND_RAW
    p.bands[3][0] = BandSlot { kind: 8, ..BandSlot::off_at(100) }; // former BAND_RAW_DATA
    p.sanitize();
    assert_eq!(p.flags, flags::ALL);
    assert_eq!((p.sub_level_cdb, p.crossover_hz, p.trim_cdb[4], p.bass_milli), (1200, 0, 600, 0), "reserved fields zeroed");
    assert_eq!(p.bands[0][0], BandSlot { kind: 1, reserved: 0, freq_hz: 20, gain_cdb: -1500, q_milli: 20_000 });
    assert!(p.bands[1][3].is_off());
    assert!(p.bands[2][11].is_off() && p.bands[3][0].is_off(), "former raw pairs are turned off");
    let mut q = Profile::defaults();
    q.crossover_hz = 0;
    assert_eq!(q.sanitized().crossover_hz, 0, "0 = stock crossover is kept");
}

#[test]
fn write_ranges_cover_every_change_within_the_limit() {
    let a = Profile::defaults();
    let mut b = a.clone();
    assert!(write_ranges(&a, &b, 55).is_empty());
    b.flags = flags::LOUDNESS;
    b.trim_cdb[0] = -100;
    for i in 0..EQ_MAX_BANDS {
        b.bands[EqTarget::Global.index()][i] = BandSlot::new(BandKind::Peak, 1000, 300, 1400);
    }
    b.bands[EqTarget::Sub.index()][11].gain_cdb = 100;
    let ranges = write_ranges(&a, &b, 55);
    assert!(ranges.iter().all(|r| !r.is_empty() && r.len() <= 55));
    let (mut bytes, target) = (a.to_bytes(), b.to_bytes());
    for r in &ranges {
        bytes[r.clone()].copy_from_slice(&target[r.clone()]);
    }
    assert_eq!(bytes, target);
    // Band ranges start and end on pair boundaries.
    for r in ranges.iter().filter(|r| r.start >= off::BANDS) {
        assert_eq!((r.start - off::BANDS) % 16, 0);
        assert_eq!((r.end - off::BANDS) % 16, 0);
    }
    assert_eq!(ranges[0], off::FLAGS..off::TRIMS + 2);
}
