pub use oc_device::console::{DeviceOptions, StandbyDelay};

/// What sets the bass module volume: the knob on its back, or the app (the profile's "Subwoofer" level). A device
/// setting of the DSP (the former bass source field of its config record).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubSource {
    /// The rear knob (stock behaviour).
    #[default]
    Knob,
    App,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceSettings {
    /// Bass module level (dB); the pod's bass knob adds to it.
    pub sub_level: f32,
    pub loudness: bool,
    /// AUX input gain (dB), mixed with USB.
    pub aux_gain: f32,
    /// Per-output level (dB): inner left, inner right, outer left, outer right, bass module.
    pub trims: [f32; 5],
    pub sub_source: SubSource,
    /// Device options (DSP profile protocol 4, `OPC`): standby delay, wake threshold, the pod's double press,
    /// headphone EQ (flat, or the active profile's global EQ).
    pub options: DeviceOptions,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            sub_level: 1.5,
            loudness: true,
            aux_gain: 0.0,
            sub_source: SubSource::Knob,
            trims: [-0.5, -0.5, 0.0, 0.0, 0.0],
            options: DeviceOptions::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileWrite {
    None,
    /// Played live only (the locked profile: its slot is never written).
    Live,
    Save,
}

/// The settings a change may set while the active profile is `locked`: the device settings and the bass module
/// level (a device setting, being moved out of the profile) change; the profile's loudness, AUX gain and trims
/// stay as they are.
pub fn guard_locked(old: &DeviceSettings, mut new: DeviceSettings, locked: bool) -> DeviceSettings {
    if locked {
        new.loudness = old.loudness;
        new.aux_gain = old.aux_gain;
        new.trims = old.trims;
    }
    new
}

/// What changing `old` into `new` (already [`guard_locked`]) writes to the active profile. `sub`: the sub
/// level is a profile field (DSP before protocol 8; after, a device setting that never writes the profile).
pub fn profile_write(old: &DeviceSettings, new: &DeviceSettings, locked: bool, sub: bool) -> ProfileWrite {
    let changed = (sub && new.sub_level != old.sub_level) || new.loudness != old.loudness || new.aux_gain != old.aux_gain || new.trims != old.trims;
    match (changed, locked) {
        (false, _) => ProfileWrite::None,
        (true, true) => ProfileWrite::Live,
        (true, false) => ProfileWrite::Save,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_locked_profile_is_never_saved_from_the_settings() {
        let old = DeviceSettings::default();
        let mut new = old.clone();
        new.loudness = !old.loudness;
        new.aux_gain = 6.0;
        new.trims[0] = 3.0;
        new.options.double_tap = !old.options.double_tap;
        // Locked: the profile fields are refused, the device option goes through, nothing is written.
        let g = guard_locked(&old, new.clone(), true);
        assert_eq!((g.loudness, g.aux_gain, g.trims), (old.loudness, old.aux_gain, old.trims));
        assert_eq!(g.options, new.options);
        assert_eq!(profile_write(&old, &g, true, true), ProfileWrite::None);
        // The bass module level stays editable: played live, never saved to the locked slot.
        let mut sub = old.clone();
        sub.sub_level = 4.0;
        let g = guard_locked(&old, sub, true);
        assert_eq!(g.sub_level, 4.0);
        assert_eq!(profile_write(&old, &g, true, true), ProfileWrite::Live);
        // A device setting (DSP protocol 8): no profile write at all, locked or not.
        assert_eq!(profile_write(&old, &g, true, false), ProfileWrite::None);
        assert_eq!(profile_write(&old, &g, false, false), ProfileWrite::None);
        assert_eq!(profile_write(&old, &guard_locked(&old, new.clone(), false), false, true), ProfileWrite::Save);
    }

    #[test]
    fn defaults_are_the_stock_options() {
        let o = DeviceSettings::default().options;
        assert_eq!((o.standby, o.double_tap, o.headphone_eq, o.threshold_step), (StandbyDelay::Stock, true, false, 0));
    }
}
