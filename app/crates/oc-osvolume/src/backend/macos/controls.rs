use super::ffi::*;
use super::props::{self, addr};
use crate::{Error, Result, VolumeState};

#[derive(Debug, Clone, Default)]
pub struct Controls {
    /// Elements with a volume control: `[0]` (main) or the stereo channels.
    pub volume: Vec<AudioObjectPropertyElement>,
    pub mute: Option<AudioObjectPropertyElement>,
}

fn out(selector: AudioObjectPropertySelector, element: u32) -> AudioObjectPropertyAddress {
    addr(selector, kAudioDevicePropertyScopeOutput, element)
}

impl Controls {
    pub fn probe(id: AudioObjectID) -> Self {
        let stereo: [u32; 2] = props::get(
            id,
            &out(
                kAudioDevicePropertyPreferredChannelsForStereo,
                kAudioObjectPropertyElementMain,
            ),
        )
        .unwrap_or([1, 2]);
        let candidates = [kAudioObjectPropertyElementMain, stereo[0], stereo[1]];
        let find = |selector| {
            let main = out(selector, kAudioObjectPropertyElementMain);
            if props::has(id, &main) && props::settable(id, &main) {
                return vec![kAudioObjectPropertyElementMain];
            }
            candidates[1..]
                .iter()
                .copied()
                .filter(|&e| props::has(id, &out(selector, e)))
                .collect()
        };
        Self {
            volume: find(kAudioDevicePropertyVolumeScalar),
            mute: find(kAudioDevicePropertyMute).first().copied(),
        }
    }

    pub fn watched(&self) -> Vec<AudioObjectPropertyAddress> {
        self.volume
            .iter()
            .map(|&e| out(kAudioDevicePropertyVolumeScalar, e))
            .chain(self.mute.map(|e| out(kAudioDevicePropertyMute, e)))
            .collect()
    }

    pub fn read(&self, id: AudioObjectID) -> Result<VolumeState> {
        let first = *self.volume.first().ok_or(Error::NoControl("volume"))?;
        let mut sum = 0.0;
        for &e in &self.volume {
            sum += props::get::<f32>(id, &out(kAudioDevicePropertyVolumeScalar, e))?;
        }
        let db = props::get::<f32>(id, &out(kAudioDevicePropertyVolumeDecibels, first))
            .unwrap_or(f32::NAN);
        let muted = match self.mute {
            Some(e) => props::get::<u32>(id, &out(kAudioDevicePropertyMute, e))? != 0,
            None => false,
        };
        Ok(VolumeState {
            scalar: sum / self.volume.len() as f32,
            db,
            muted,
        })
    }

    pub fn set_scalar(&self, id: AudioObjectID, scalar: f32) -> Result<()> {
        if self.volume.is_empty() {
            return Err(Error::NoControl("volume"));
        }
        for &e in &self.volume {
            props::set(id, &out(kAudioDevicePropertyVolumeScalar, e), scalar)?;
        }
        Ok(())
    }

    pub fn set_mute(&self, id: AudioObjectID, muted: bool) -> Result<()> {
        let e = self.mute.ok_or(Error::NoControl("mute"))?;
        props::set(id, &out(kAudioDevicePropertyMute, e), muted as u32)
    }
}
