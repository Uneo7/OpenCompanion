use oc_device::params::Profile as Params;
use oc_device::worker::Command;

use super::convert;
use super::state::HwState;
use super::HwDevice;
use crate::model::eq::ProfileBands;
use crate::model::profile::{Profile, DEFAULT_NAMES, HUE_CHOICES, LOCKED, NAME_MAX, PROFILE_COUNT};

/// The UI list: names and colours from the slot directory (`OPN`), the sound of the active slot from the
/// speaker, the others as last seen this session (the DSP only exposes its active profile). An empty slot is
/// shown as the firmware treats it: the default name and colour, Bose base, flat EQ.
pub fn list(st: &HwState, bands: &[Option<ProfileBands>; PROFILE_COUNT]) -> Vec<Profile> {
    (0..PROFILE_COUNT)
        .map(|i| {
            let slot = st.slots.get(i).filter(|s| s.valid);
            Profile {
                name: slot.map_or_else(|| DEFAULT_NAMES[i].to_owned(), |s| s.name.clone()),
                hue: usize::from(slot.map_or(i as u8, |s| s.color)) % HUE_CHOICES.len(),
                bands: bands[i].clone().unwrap_or_else(ProfileBands::bose),
                locked: i == LOCKED,
            }
        })
        .collect()
}

pub fn slot_empty(st: &HwState, i: usize) -> bool {
    !st.slots.get(i).is_some_and(|s| s.valid)
}

impl HwDevice {
    pub(super) fn activate(&mut self, i: usize) {
        if i >= PROFILE_COUNT || !self.st.ready() {
            return;
        }
        self.ui_active = i;
        self.requested = Some(i);
        self.compare = false;
        self.send(Command::Load(i as u8));
        self.rebuild();
    }

    /// Saves `bands` to slot `i` (the active one: the DSP saves its active profile only).
    pub(super) fn store(&mut self, i: usize, bands: &ProfileBands) -> bool {
        if i != self.ui_active || i >= self.profiles.len() || i == LOCKED {
            return false;
        }
        let (Some(stored), Some(live)) = (self.stored.as_mut(), self.live.as_mut()) else { return false };
        let fits = convert::apply_bands(stored, bands);
        stored.sanitize();
        convert::apply_bands(live, bands);
        live.sanitize();
        if !fits {
            self.notice(crate::i18n::tr("eq.cut"));
        }
        self.bands[i] = Some(bands.clone());
        self.save_settings_at = None;
        self.save_stored();
        self.rebuild();
        true
    }

    /// Profile `i` as the speaker stores it: the exact active profile when it is loaded, else built from the
    /// app's copy (see [`crate::model::device::params_of`]).
    pub(super) fn params_of(&self, i: usize) -> Option<Params> {
        match &self.stored {
            Some(p) if i == self.ui_active && !self.compare => Some(p.clone()),
            _ => None,
        }
    }

    pub(super) fn import(&mut self, i: usize, p: &Params) -> bool {
        if i != self.ui_active || i >= self.profiles.len() || i == LOCKED {
            return false;
        }
        let (Some(stored), Some(live)) = (self.stored.as_mut(), self.live.as_mut()) else { return false };
        let mut new = p.clone();
        new.name = stored.name;
        new.color = stored.color;
        new.set_flag(oc_device::params::flags::BYPASS, false);
        // Every profile uses the Bose base, also one imported from an older file on the neutral base.
        new.set_flag(oc_device::params::flags::STOCK_GRAPH, true);
        new.sanitize();
        let bypass = live.has_flag(oc_device::params::flags::BYPASS);
        *stored = new.clone();
        new.set_flag(oc_device::params::flags::BYPASS, bypass);
        *live = new;
        self.bands[i] = Some(convert::bands_from(stored, None));
        self.save_settings_at = None;
        self.save_stored();
        self.rebuild();
        self.reloaded = true;
        true
    }

    /// See [`crate::model::Device::duplicate`]: the worker copies the slot in the speaker; the app shows the
    /// copy at once (name, colour, bands) and makes it active.
    pub(super) fn duplicate_slot(&mut self, from: usize, to: usize, name: &str, hue: usize) -> bool {
        if from >= PROFILE_COUNT || to >= PROFILE_COUNT || from == to || to == LOCKED || !self.st.ready() {
            return false;
        }
        let cut = oc_device::params::name_from_bytes(&oc_device::params::name_to_bytes(name));
        let color = (hue % HUE_CHOICES.len()) as u8;
        self.send(Command::Duplicate { from: from as u8, to: to as u8, name: cut.clone(), color });
        self.bands[to] = self.bands[from].clone();
        if let Some(e) = self.st.slots.get_mut(to) {
            e.name = cut;
            e.color = color;
            e.valid = true;
        }
        self.ui_active = to;
        self.requested = Some(to);
        self.compare = false;
        self.rebuild();
        true
    }

    pub(super) fn retag(&mut self, i: usize, name: Option<String>, color: Option<u8>) {
        if i >= PROFILE_COUNT || !self.st.ready() {
            return;
        }
        if i == self.ui_active {
            let (Some(stored), Some(live)) = (self.stored.as_mut(), self.live.as_mut()) else { return };
            for p in [stored, live] {
                if let Some(n) = &name {
                    p.set_name(n);
                }
                if let Some(c) = color {
                    p.color = c;
                }
            }
            self.save_stored();
        } else if let Some(live) = self.live.clone() {
            let active = self.st.info.and_then(|x| x.active);
            self.send(Command::Patch { slot: i as u8, name: name.clone(), color, active, live });
        }
        // Shown at once; the slot directory read after the save confirms it.
        if let Some(e) = self.st.slots.get_mut(i) {
            if let Some(n) = name {
                e.name = n;
            }
            if let Some(c) = color {
                e.color = c;
            }
            e.valid = true;
        }
        self.rebuild();
    }

    pub(super) fn rename(&mut self, i: usize, name: &str) {
        let name: String = name.trim().chars().take(NAME_MAX).collect();
        if name.is_empty() || i >= self.profiles.len() || i == LOCKED {
            return;
        }
        // The DSP keeps 16 bytes of UTF-8.
        let cut = oc_device::params::name_from_bytes(&oc_device::params::name_to_bytes(&name));
        if cut != name {
            self.notice(crate::i18n::t_with("profile.rename.cut", &[("name", &cut)]));
        }
        self.retag(i, Some(cut), None);
    }

    pub(super) fn set_hue(&mut self, i: usize, hue: usize) {
        if i == LOCKED {
            return;
        }
        self.retag(i, None, Some((hue % HUE_CHOICES.len()) as u8));
    }

    pub(super) fn preview(&mut self, bands: Option<&ProfileBands>) {
        let (Some(stored), Some(live)) = (&self.stored, self.live.as_mut()) else { return };
        let bypass = live.has_flag(oc_device::params::flags::BYPASS);
        *live = stored.clone();
        if let Some(b) = bands {
            convert::apply_bands(live, b);
        }
        live.set_flag(oc_device::params::flags::BYPASS, bypass);
        live.sanitize();
        if !self.compare {
            self.play();
        }
    }

    /// Listens to the built-in Bose profile (`OPK`), or back to `live`.
    pub(super) fn compare(&mut self, on: bool) {
        if on == self.compare || !self.st.ready() {
            return;
        }
        self.compare = on;
        if on {
            self.send(Command::LoadBose);
        } else {
            self.play();
        }
    }

    pub(super) fn bypass(&mut self, on: bool) {
        if let Some(l) = self.live.as_mut() {
            l.set_flag(oc_device::params::flags::BYPASS, on);
        }
        if !self.compare {
            self.play();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oc_device::console::SlotEntry;

    #[test]
    fn slots_become_profiles() {
        let st = HwState { slots: vec![
            SlotEntry { valid: true, color: 0, seq: 1, name: "Bose d'origine".into() },
            SlotEntry { valid: true, color: 13, seq: 2, name: "Bureau".into() },
            SlotEntry::default(),
            SlotEntry::default(),
        ], ..Default::default() };
        let bands: [Option<ProfileBands>; PROFILE_COUNT] = Default::default();
        let l = list(&st, &bands);
        assert_eq!(l.len(), PROFILE_COUNT);
        assert_eq!((l[1].name.as_str(), l[1].hue), ("Bureau", 3));
        // Empty slots: default name and colour, Bose base, flat EQ.
        assert_eq!((l[2].name.as_str(), l[2].hue), ("Profil 3", 2));
        assert!(l.iter().all(|p| p.bands == ProfileBands::bose()));
        assert!(slot_empty(&st, 3) && !slot_empty(&st, 0));
    }
}
