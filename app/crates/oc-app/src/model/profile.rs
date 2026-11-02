use super::eq::{Base, ProfileBands};

/// Exactly four profiles live in the speaker; a double press on Mute cycles 1 → 4 → 1.
pub const PROFILE_COUNT: usize = 4;
/// Slot of "Bose® d'origine": read-only (never saved, renamed or recoloured; the worker refuses it too).
pub const LOCKED: usize = oc_device::worker::LOCKED_SLOT as usize;

pub const NAME_MAX: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hue {
    pub a: [u8; 3],
    pub b: [u8; 3],
}

const fn hex(v: u32) -> [u8; 3] {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

const fn hue(a: u32, b: u32) -> Hue {
    Hue { a: hex(a), b: hex(b) }
}

pub const HUE_CHOICES: [Hue; 10] = [
    hue(0xffb547, 0xff6f59),
    hue(0x22d3b8, 0x3b82ff),
    hue(0xb46bff, 0xff4fa3),
    hue(0x7b6cff, 0x2fc4ff),
    hue(0x9be22f, 0x22d3ee),
    hue(0xff8a4c, 0xffcc4d),
    hue(0xff5f8f, 0xffb84d),
    hue(0x4ade80, 0x38bdf8),
    hue(0xf472b6, 0xa78bfa),
    hue(0xfb923c, 0xf43f5e),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub name: String,
    pub hue: usize,
    pub bands: ProfileBands,
    /// "Bose® d'origine" ([`LOCKED`]): shown read-only, no write is ever sent for it.
    pub locked: bool,
}

impl Profile {
    pub fn hue(&self) -> Hue {
        HUE_CHOICES[self.hue % HUE_CHOICES.len()]
    }

    pub fn base(&self) -> Base {
        self.bands.base
    }
}

/// French default names of the four profiles, as stored in the speaker (shown translated,
/// `profile.default.p1..p4`).
pub const DEFAULT_NAMES: [&str; PROFILE_COUNT] = ["Bose d'origine", "Profil 2", "Profil 3", "Profil 4"];
/// Default names written by earlier versions, still shown as the default (translated) name.
pub const LEGACY_DEFAULT_NAMES: [&str; PROFILE_COUNT] = ["Bose d'origine", "Bureau", "Films nuit", "Musique"];

/// A new speaker's profiles, and the simulated speaker's: the four default names and colours, each on the
/// Bose base with a flat EQ (as the firmware treats an empty slot).
pub fn default_profiles() -> [Profile; PROFILE_COUNT] {
    std::array::from_fn(|i| Profile { name: DEFAULT_NAMES[i].into(), hue: i, bands: ProfileBands::bose(), locked: i == LOCKED })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_default_profiles_on_the_bose_base() {
        let ps = default_profiles();
        assert_eq!(ps.len(), PROFILE_COUNT);
        assert!(ps.iter().all(|p| p.base() == Base::Bose && p.bands == ProfileBands::bose()));
        assert_eq!((ps[0].name.as_str(), ps[3].hue), ("Bose d'origine", 3));
        assert_eq!(ps.iter().map(|p| p.locked).collect::<Vec<_>>(), [true, false, false, false]);
    }

    #[test]
    fn hue_wraps() {
        let p = Profile { name: "x".into(), hue: 13, bands: ProfileBands::flat(), locked: false };
        assert_eq!(p.hue(), HUE_CHOICES[3]);
        assert_eq!(HUE_CHOICES[3].a, [0x7b, 0x6c, 0xff]);
    }
}
