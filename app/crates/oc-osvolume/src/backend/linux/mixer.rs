use alsa::mixer::{MilliBel, Mixer, Selem, SelemChannelId, SelemId};
use alsa::poll::{pollfd, Descriptors};
use alsa::Round;

use crate::convert::{db_to_scalar, raw_to_scalar, scalar_to_db, scalar_to_raw};
use crate::{Error, Result, VolumeState};

const PREFERRED: [&str; 2] = ["PCM", "Master"];

/// ALSA reports a "mute" bottom of range (SND_CTL_TLV_DB_GAIN_MUTE) as
/// -9999999 centi-dB; anything this low means silence.
const MUTE_FLOOR_CB: i64 = -9_999_000;

pub struct Control {
    ctl: String,
    mixer: Mixer,
    selem: SelemId,
}

fn selem_id(selem: &Selem) -> Option<SelemId> {
    let id = selem.get_id();
    Some(SelemId::new(id.get_name().ok()?, id.get_index()))
}

fn open_mixer(ctl: &str) -> Result<Mixer> {
    Mixer::new(ctl, false).map_err(|e| Error::os("snd_mixer_open", e))
}

fn pick_element(mixer: &Mixer) -> Option<SelemId> {
    PREFERRED
        .iter()
        .filter_map(|name| mixer.find_selem(&SelemId::new(name, 0)))
        .chain(mixer.iter().filter_map(Selem::new))
        .find(|s| s.has_playback_volume())
        .and_then(|s| selem_id(&s))
}

impl Control {
    pub fn open(ctl: &str) -> Result<Self> {
        let mixer = open_mixer(ctl)?;
        let selem = pick_element(&mixer).ok_or(Error::NoControl("volume"))?;
        Ok(Self {
            ctl: ctl.to_owned(),
            mixer,
            selem,
        })
    }

    pub fn poll_fds(&self) -> Result<Vec<pollfd>> {
        Descriptors::get(&self.mixer).map_err(|e| Error::os("snd_mixer_poll_descriptors", e))
    }

    pub fn ctl_name(&self) -> &str {
        &self.ctl
    }

    fn selem(&self) -> Result<Selem<'_>> {
        // Refresh cached values with pending events (other writers, knob).
        self.mixer
            .handle_events()
            .map_err(|e| Error::os("snd_mixer_handle_events", e))?;
        self.mixer
            .find_selem(&self.selem)
            .ok_or_else(|| Error::os("alsa", "mixer element disappeared"))
    }

    pub fn read(&self) -> Result<VolumeState> {
        read_selem(&self.selem()?)
    }

    pub fn set_scalar(&self, scalar: f32) -> Result<()> {
        let s = self.selem()?;
        match db_range(&s) {
            Some((min, max, mute_floor)) => {
                let db = scalar_to_db(scalar, min, max, mute_floor);
                s.set_playback_db_all(MilliBel((db * 100.0).round() as i64), Round::Floor)
                    .map_err(|e| Error::os("snd_mixer_selem_set_playback_dB_all", e))
            }
            None => {
                let (min, max) = s.get_playback_volume_range();
                s.set_playback_volume_all(scalar_to_raw(scalar, min, max))
                    .map_err(|e| Error::os("snd_mixer_selem_set_playback_volume_all", e))
            }
        }
    }

    pub fn set_mute(&self, muted: bool) -> Result<()> {
        let s = self.selem()?;
        if !s.has_playback_switch() {
            return Err(Error::NoControl("mute"));
        }
        // ALSA switch semantics: 1 = sound on.
        s.set_playback_switch_all(i32::from(!muted))
            .map_err(|e| Error::os("snd_mixer_selem_set_playback_switch_all", e))
    }
}

/// (min dB, max dB, bottom means mute), or None without usable dB info.
fn db_range(s: &Selem) -> Option<(f32, f32, bool)> {
    let (min, max) = s.get_playback_db_range();
    if max.0 <= min.0 {
        return None;
    }
    if min.0 <= MUTE_FLOOR_CB {
        // Use the first real step as the bottom of the curve.
        let (raw_min, _) = s.get_playback_volume_range();
        let floor = s.ask_playback_vol_db(raw_min + 1).ok()?;
        return Some((floor.to_db(), max.to_db(), true));
    }
    Some((min.to_db(), max.to_db(), false))
}

pub fn read_selem(s: &Selem) -> Result<VolumeState> {
    // Front left, which is also the channel of a mono element.
    let ch = SelemChannelId::mono();
    let raw = s
        .get_playback_volume(ch)
        .map_err(|e| Error::os("snd_mixer_selem_get_playback_volume", e))?;
    let (raw_min, raw_max) = s.get_playback_volume_range();
    let db = s
        .get_playback_vol_db(ch)
        .map(MilliBel::to_db)
        .unwrap_or(f32::NAN);
    let scalar = match db_range(s) {
        Some((min, max, mute_floor)) if db.is_finite() => db_to_scalar(db, min, max, mute_floor),
        _ => raw_to_scalar(raw, raw_min, raw_max),
    };
    let muted =
        s.has_playback_switch() && s.get_playback_switch(ch).map(|v| v == 0).unwrap_or(false);
    Ok(VolumeState { scalar, db, muted })
}
