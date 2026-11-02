//! The six UI lists are the DSP's six targets, one to one ([`Target::index`] = `EqTarget` index);
//! - a disabled band keeps its type and values (`type | BAND_DISABLED`, DSP profile protocol 4). Older
//!   firmware stored it as a `BAND_OFF` slot keeping its frequency / gain / Q; the firmware zeroes the reserved
//!   byte, so the shape of a disabled band is not stored: it comes back as a bell unless `hint` (the bands the
//!   app last wrote) has the same disabled band at that place;
//! - unused slots are written as `params_defaults()` writes them; trailing off slots at 0 dB / Q 0.707 read as
//!   unused;
//! - the base is `PF_STOCK_GRAPH` (Bose base) or not (neutral base);
//! - raw biquad pairs (copies of the former built-in "Bose d'origine") have no UI form and are ignored by the
//!   firmware on the Bose base: such a profile reads as the Bose base with a flat EQ (the next save clears them).

use oc_device::params::{self as p, BandKind, BandSlot, EqTarget, DEFAULT_FREQS, EQ_MAX_BANDS};

use crate::model::eq::{Band, BandType, Base, ProfileBands, Target};
use crate::model::settings::DeviceSettings;

fn eq_target(t: Target) -> EqTarget {
    EqTarget::ALL[t.index()]
}

fn kind_byte(t: BandType) -> BandKind {
    match t {
        BandType::Peak => BandKind::Peak,
        BandType::LowShelf => BandKind::LowShelf,
        BandType::HighShelf => BandKind::HighShelf,
        BandType::HighPass => BandKind::HighPass,
        BandType::LowPass => BandKind::LowPass,
        BandType::Notch => BandKind::Notch,
    }
}

fn band_type(k: BandKind) -> Option<BandType> {
    Some(match k {
        BandKind::Peak => BandType::Peak,
        BandKind::LowShelf => BandType::LowShelf,
        BandKind::HighShelf => BandType::HighShelf,
        BandKind::HighPass => BandType::HighPass,
        BandKind::LowPass => BandType::LowPass,
        BandKind::Notch => BandType::Notch,
        BandKind::Off => return None,
    })
}

/// A band as a slot; a disabled band keeps its type and values (`BAND_DISABLED`, DSP profile protocol 4; an
/// older DSP turns such a slot off, as it did before).
fn slot_of(b: &Band) -> BandSlot {
    let kind = kind_byte(b.kind);
    let (flo, fhi) = p::limits::FREQ_HZ;
    let (glo, ghi) = p::limits::GAIN_CDB;
    let (qlo, qhi) = p::limits::Q_MILLI;
    let mut s = BandSlot::new(
        kind,
        b.freq.round().clamp(f64::from(flo), f64::from(fhi)) as u16,
        (b.gain * 100.0).round().clamp(f64::from(glo), f64::from(ghi)) as i16,
        (b.q * 1000.0).round().clamp(f64::from(qlo), f64::from(qhi)) as u16,
    );
    s.set_enabled(b.on);
    s
}

fn band_of(s: &BandSlot, hint: Option<&Band>) -> Band {
    let (freq, gain, q) = (f64::from(s.freq_hz), f64::from(s.gain_cdb) / 100.0, f64::from(s.q_milli) / 1000.0);
    match s.kept_kind().and_then(band_type) {
        Some(kind) if s.is_enabled() => Band::new(kind, freq, gain, q),
        Some(kind) => Band::new(kind, freq, gain, q).off(),
        // A legacy `BAND_OFF` slot: its type was not stored; the last known one, else a bell.
        None => {
            let kind = hint
                .filter(|h| !h.on && slot_of(h) == *s)
                .map_or(BandType::Peak, |h| h.kind);
            Band::new(kind, freq, gain, q).off()
        }
    }
}

/// An off slot at 0 dB / Q 0.707: nothing the user made.
fn blank(s: &BandSlot) -> bool {
    s.is_off() && s.gain_cdb == 0 && s.q_milli == 707
}

fn list_of(slots: &[BandSlot; EQ_MAX_BANDS], hint: Option<&Vec<Band>>) -> Vec<Band> {
    let used = slots.iter().rposition(|s| !blank(s)).map_or(0, |i| i + 1);
    slots[..used].iter().enumerate().map(|(i, s)| band_of(s, hint.and_then(|h| h.get(i)))).collect()
}

fn write_list(slots: &mut [BandSlot; EQ_MAX_BANDS], bands: impl Iterator<Item = Band>) {
    let mut n = 0;
    for b in bands.take(EQ_MAX_BANDS) {
        slots[n] = slot_of(&b);
        n += 1;
    }
    for (i, s) in slots.iter_mut().enumerate().skip(n) {
        *s = BandSlot::off_at(DEFAULT_FREQS[i]);
    }
}

/// The UI sound of `prof`. `hint` = the bands the app last had for this profile (keeps disabled band shapes).
pub fn bands_from(prof: &p::Profile, hint: Option<&ProfileBands>) -> ProfileBands {
    let mut out = ProfileBands::flat();
    // Every profile uses the Bose base: one saved on the neutral base (older app) is read, and rewritten, on it.
    out.base = Base::Bose;
    for t in Target::ALL {
        *out.get_mut(t) = list_of(prof.target(eq_target(t)), hint.map(|h| h.get(t)));
    }
    // EQ mode: the PF_ADVANCED bit; a DSP firmware that masks it loses it, so it is also inferred (an output
    // has bands) or remembered from what the app last had for this slot (an empty Advanced profile).
    out.advanced = prof.has_flag(p::flags::ADVANCED) || out.inferred_advanced() || hint.is_some_and(|h| h.advanced);
    // Advanced has no global list: one left over (older app) goes in front of each output.
    if out.advanced && !out.get(Target::Global).is_empty() {
        out.to_advanced();
    }
    out
}

/// Writes `bands` into `prof` (the base flag and the band slots). Lists longer than the DSP's 12 slots are cut;
/// returns false when something was cut.
pub fn apply_bands(prof: &mut p::Profile, bands: &ProfileBands) -> bool {
    // The app has no base choice any more: always the Bose base (whatever `bands.base` says).
    prof.set_flag(p::flags::STOCK_GRAPH, true);
    prof.set_flag(p::flags::ADVANCED, bands.advanced);
    let mut fits = true;
    for t in Target::ALL {
        fits &= bands.get(t).len() <= EQ_MAX_BANDS;
        write_list(prof.target_mut(eq_target(t)), bands.get(t).iter().copied());
    }
    fits
}

/// The profile fields of the settings (the rest of `base` is kept: those settings are not stored in the speaker).
/// `sub`: the sub level is a profile field (DSP before protocol 8; after, a device setting).
pub fn settings_from(prof: &p::Profile, base: &DeviceSettings, sub: bool) -> DeviceSettings {
    let mut s = base.clone();
    if sub {
        s.sub_level = f32::from(prof.sub_level_cdb) / 100.0;
    }
    s.loudness = prof.has_flag(p::flags::LOUDNESS);
    s.aux_gain = f32::from(prof.aux_gain_cdb) / 100.0;
    for (t, c) in s.trims.iter_mut().zip(prof.trim_cdb) {
        *t = f32::from(c) / 100.0;
    }
    s
}

fn cdb(db: f32) -> i16 {
    (db * 100.0).round().clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
}

pub fn apply_settings(prof: &mut p::Profile, s: &DeviceSettings, sub: bool) {
    if sub {
        prof.sub_level_cdb = cdb(s.sub_level);
    }
    // The crossover is not exposed: always the stock one.
    prof.crossover_hz = 0;
    prof.set_flag(p::flags::LOUDNESS, s.loudness);
    prof.aux_gain_cdb = cdb(s.aux_gain);
    for (c, t) in prof.trim_cdb.iter_mut().zip(s.trims) {
        *c = cdb(t);
    }
    prof.sanitize();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ProfileBands {
        let mut b = ProfileBands::flat();
        *b.get_mut(Target::Global) = vec![
            Band::new(BandType::HighPass, 32.0, 0.0, 0.71),
            Band::new(BandType::LowShelf, 105.0, 3.0, 0.7),
            Band::new(BandType::Notch, 6200.0, 1.0, 1.0).off(),
            Band::new(BandType::HighShelf, 9500.0, 2.0, 0.7),
        ];
        b
    }

    fn sample_advanced() -> ProfileBands {
        let mut b = ProfileBands::bose();
        b.advanced = true;
        *b.get_mut(Target::InnerLeft) = vec![Band::new(BandType::Peak, 1200.0, -1.0, 1.8)];
        *b.get_mut(Target::OuterRight) = vec![Band::new(BandType::HighShelf, 8000.0, 1.0, 0.7)];
        *b.get_mut(Target::Sub) = vec![Band::new(BandType::LowPass, 160.0, 0.0, 0.71)];
        b
    }

    #[test]
    fn the_eq_mode_round_trips_and_is_inferred_without_the_flag() {
        let b = sample_advanced();
        let mut prof = p::Profile::defaults();
        assert!(apply_bands(&mut prof, &b));
        let prof = prof.sanitized();
        assert!(prof.has_flag(p::flags::ADVANCED));
        assert_eq!(prof.target(EqTarget::OuterR)[0].freq_hz, 8000);
        assert_eq!(prof.target(EqTarget::InnerL)[0].freq_hz, 1200);
        assert_eq!(prof.target(EqTarget::Sub)[0].freq_hz, 160);
        assert!(prof.target(EqTarget::InnerR)[0].is_off());
        assert_eq!(bands_from(&prof, None), b);
        // A DSP firmware that masks the bit: inferred from the output bands.
        let mut masked = prof.clone();
        masked.set_flag(p::flags::ADVANCED, false);
        assert!(bands_from(&masked, None).advanced);
        // An empty Advanced profile: remembered from what the app last had for the slot.
        let mut empty = ProfileBands::bose();
        empty.advanced = true;
        let mut prof = p::Profile::defaults();
        apply_bands(&mut prof, &empty);
        prof.set_flag(p::flags::ADVANCED, false);
        assert!(!bands_from(&prof, None).advanced);
        assert!(bands_from(&prof, Some(&empty)).advanced);
    }

    #[test]
    fn bands_round_trip_one_list_per_dsp_target() {
        let b = sample();
        let mut prof = p::Profile::defaults();
        assert!(apply_bands(&mut prof, &b));
        let prof = prof.sanitized();
        assert!(!prof.has_flag(p::flags::ADVANCED));
        assert!(prof.target(EqTarget::InnerR)[0].is_off());
        assert!(prof.target(EqTarget::Global)[2].is_disabled());
        // A disabled band keeps its type and values in the speaker: no hint needed.
        assert_eq!(bands_from(&prof, Some(&b)), b);
        assert_eq!(bands_from(&prof, None), b);
        // A legacy BAND_OFF slot (older DSP) reads back as a disabled bell.
        let mut legacy = prof.clone();
        legacy.bands[0][2].kind = BandKind::Off.byte();
        let cold = bands_from(&legacy, None);
        assert_eq!(cold.get(Target::Global)[2].kind, BandType::Peak);
        assert!(!cold.get(Target::Global)[2].on);
    }

    #[test]
    fn defaults_read_as_flat_and_overflow_is_cut() {
        // `params_defaults()` (neutral base) reads as the Bose base.
        assert_eq!(bands_from(&p::Profile::defaults(), None), ProfileBands::flat());
        let mut b = ProfileBands::flat();
        *b.get_mut(Target::OuterLeft) = vec![Band::new(BandType::Peak, 200.0, 1.0, 1.0); 16];
        let mut prof = p::Profile::defaults();
        assert!(!apply_bands(&mut prof, &b));
        assert_eq!(prof.target(EqTarget::OuterL).iter().filter(|s| !s.is_off()).count(), EQ_MAX_BANDS);
    }

    #[test]
    fn former_raw_profiles_read_as_the_bose_base_and_the_base_round_trips() {
        let mut prof = p::Profile::defaults();
        prof.bands[0][0] = BandSlot { kind: 7, ..BandSlot::off_at(1000) }; // former BAND_RAW: turned off
        prof.sanitize();
        assert_eq!(bands_from(&prof, None), ProfileBands::bose());
        let mut prof = p::Profile::defaults();
        assert!(apply_bands(&mut prof, &ProfileBands::bose()));
        assert!(prof.has_flag(p::flags::STOCK_GRAPH));
        assert_eq!(bands_from(&prof, None), ProfileBands::bose());
        apply_bands(&mut prof, &ProfileBands::flat());
        assert!(prof.has_flag(p::flags::STOCK_GRAPH));
    }

    #[test]
    fn every_profile_the_app_writes_is_on_the_bose_base() {
        assert_eq!(ProfileBands::default().base, Base::Bose);
        assert_eq!(ProfileBands::flat(), ProfileBands::bose());
        // A neutral profile (older app, old .ocprofile, params_defaults) reads and writes back on the Bose base.
        let mut neutral = p::Profile::defaults();
        neutral.set_flag(p::flags::STOCK_GRAPH, false);
        let b = bands_from(&neutral, None);
        assert_eq!(b.base, Base::Bose);
        let mut out = neutral.clone();
        let mut old = b;
        old.base = Base::Neutral;
        apply_bands(&mut out, &old);
        assert!(out.has_flag(p::flags::STOCK_GRAPH));
        assert!(crate::model::profile::default_profiles().iter().all(|p| p.base() == Base::Bose));
    }

    #[test]
    fn settings_round_trip() {
        let s = DeviceSettings { sub_level: -2.5, loudness: false, aux_gain: 3.0, ..Default::default() };
        let mut prof = p::Profile::defaults();
        prof.crossover_hz = 120;
        apply_settings(&mut prof, &s, true);
        // The stock crossover is forced: the setting is no longer exposed.
        assert_eq!((prof.sub_level_cdb, prof.crossover_hz, prof.aux_gain_cdb), (-250, 0, 300));
        assert_eq!(prof.trim_cdb[0], -50);
        assert_eq!(settings_from(&prof, &DeviceSettings::default(), true), s);
        // DSP protocol 8: the sub level is a device setting, never read from nor written to a profile.
        let mut other = s.clone();
        other.sub_level = 6.0;
        apply_settings(&mut prof, &other, false);
        assert_eq!(prof.sub_level_cdb, -250);
        assert_eq!(settings_from(&prof, &other, false).sub_level, 6.0);
    }
}
