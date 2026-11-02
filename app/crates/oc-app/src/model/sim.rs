use super::device::{Device, DeviceMode, FirmwarePart, LinkState, Status, StreamFormat, Telemetry, UpdateProgress};
use super::eq::ProfileBands;
use super::profile::{default_profiles, Profile, NAME_MAX, PROFILE_COUNT};
use super::settings::DeviceSettings;

/// Duration of a simulated firmware update (s): 4 % every 110 ms.
const UPDATE_S: f64 = 2.75;
/// i18n keys of the simulated update steps (step 1 depends on the chip).
const UPDATE_STEPS: [&str; 5] = ["fw.sim.backup", "", "fw.sim.write", "fw.sim.verify", "fw.sim.restart"];

pub struct SimDevice {
    now: f64,
    volume: u8,
    muted: bool,
    profiles: Vec<Profile>,
    active: usize,
    settings: DeviceSettings,
    telemetry: Telemetry,
    next_sample: f64,
    seed: u32,
    updates: [Option<f64>; 2],
    pub previewing: bool,
    pub compare: bool,
    pub bypass: bool,
    backup_date: String,
    mode: DeviceMode,
    /// Connection to report instead of `Simulated` (onboarding scenarios: Bose firmware, mixed, recovery…).
    pub connection: Option<super::device::Connection>,
    /// Update check to report (scenarios, screenshots): the simulated speaker is up to date otherwise.
    pub update_check: Option<oc_device::install::UpdateCheck>,
    pub app_update: Option<oc_device::install::AppUpdate>,
    restart: Option<(FirmwarePart, f64)>,
    notice: Option<String>,
}

impl Default for SimDevice {
    fn default() -> Self {
        Self::new()
    }
}

const SIM_RESTART_S: f64 = 3.0;

impl SimDevice {
    pub fn new() -> Self {
        Self {
            now: 0.0,
            volume: 42,
            muted: false,
            profiles: default_profiles().to_vec(),
            active: 0,
            settings: DeviceSettings::default(),
            telemetry: Telemetry { dsp_load: 38.0, supply: 13.41, thermal_trip: false, clips: (3, 0), faults: 0 },
            next_sample: 1.0,
            seed: 0x5eed,
            updates: [None, None],
            previewing: false,
            compare: false,
            bypass: false,
            backup_date: "12/10/2026".into(),
            mode: DeviceMode::Surround51,
            connection: None,
            update_check: None,
            app_update: None,
            restart: None,
            notice: None,
        }
    }

    fn random(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        ((self.seed >> 16) & 0x7fff) as f32 / 32_768.0
    }

    fn slot(part: FirmwarePart) -> usize {
        match part {
            FirmwarePart::Usb => 0,
            FirmwarePart::Dsp => 1,
        }
    }
}

impl Device for SimDevice {
    fn tick(&mut self, now: f64) {
        self.now = now;
        if let Some((part, _)) = self.restart.filter(|(_, s)| now - s >= SIM_RESTART_S) {
            self.restart = None;
            let key = if part == FirmwarePart::Dsp { "settings.advanced.restart.done_dsp" } else { "settings.advanced.restart.done_usb" };
            self.notice = Some(crate::i18n::tr(key).into());
        }
        if now >= self.next_sample {
            self.next_sample = now + 1.0;
            let d = (self.random() - 0.5) * 4.0;
            self.telemetry.dsp_load = (self.telemetry.dsp_load + d).clamp(24.0, 62.0);
            self.telemetry.supply = 13.38 + self.random() * 0.06;
        }
        for u in &mut self.updates {
            if u.is_some_and(|start| now - start >= UPDATE_S) {
                *u = None;
            }
        }
    }

    fn connection(&self) -> super::device::Connection {
        self.connection.clone().unwrap_or(super::device::Connection::Simulated)
    }

    fn status(&self) -> Status {
        Status {
            link: LinkState::Connected,
            stream: match self.mode {
                DeviceMode::Surround51 => StreamFormat::Surround51,
                DeviceMode::Hq21 => StreamFormat::Hq21,
            },
            firmware_usb: "1.2.0".into(),
            firmware_dsp: "0.9.3".into(),
            pod_present: true,
            pod_knob_ok: true,
            pod_bass: 2,
            headphones: false,
        }
    }

    fn telemetry(&self) -> Telemetry {
        self.telemetry
    }

    fn reset_clips(&mut self) {
        self.telemetry.clips = (0, 0);
    }

    fn volume(&self) -> u8 {
        self.volume
    }

    fn set_volume(&mut self, v: u8) {
        self.volume = v.min(100);
    }

    fn muted(&self) -> bool {
        self.muted
    }

    fn set_muted(&mut self, m: bool) {
        self.muted = m;
    }

    fn profiles(&self) -> &[Profile] {
        &self.profiles
    }

    fn active_profile(&self) -> usize {
        self.active
    }

    fn set_active_profile(&mut self, i: usize) {
        if i < PROFILE_COUNT {
            self.active = i;
            self.previewing = false;
        }
    }

    fn duplicate(&mut self, from: usize, to: usize, name: &str, hue: usize) -> bool {
        let n = self.profiles.len();
        if from >= n || to >= n || from == to || self.profiles[to].locked {
            return false;
        }
        let bands = self.profiles[from].bands.clone();
        let p = &mut self.profiles[to];
        p.bands = bands;
        p.name = name.chars().take(NAME_MAX).collect();
        p.hue = hue;
        self.active = to;
        self.previewing = false;
        true
    }

    fn store_profile(&mut self, i: usize, bands: &ProfileBands) -> bool {
        match self.profiles.get_mut(i).filter(|p| !p.locked) {
            Some(p) => {
                p.bands = bands.clone();
                true
            }
            _ => false,
        }
    }

    fn rename_profile(&mut self, i: usize, name: &str) {
        let name: String = name.trim().chars().take(NAME_MAX).collect();
        if let Some(p) = self.profiles.get_mut(i).filter(|p| !name.is_empty() && !p.locked) {
            p.name = name;
        }
    }

    fn set_profile_hue(&mut self, i: usize, hue: usize) {
        if let Some(p) = self.profiles.get_mut(i).filter(|p| !p.locked) {
            p.hue = hue;
        }
    }

    fn preview(&mut self, bands: Option<&ProfileBands>) {
        self.previewing = bands.is_some();
    }

    fn set_compare(&mut self, on: bool) {
        self.compare = on;
    }

    fn set_bypass(&mut self, on: bool) {
        self.bypass = on;
    }

    fn rear_knob_db(&self) -> Option<f32> {
        Some(3.0)
    }

    fn mode(&self) -> Option<DeviceMode> {
        Some(self.mode)
    }

    fn can_set_mode(&self) -> bool {
        true
    }

    fn set_mode(&mut self, mode: DeviceMode) {
        self.mode = mode;
    }

    fn settings(&self) -> &DeviceSettings {
        &self.settings
    }

    fn restart(&mut self, part: FirmwarePart) {
        if self.restart.is_none() {
            self.restart = Some((part, self.now));
        }
    }

    fn restarting(&self) -> Option<FirmwarePart> {
        self.restart.map(|(p, _)| p)
    }

    fn updates(&mut self) -> oc_device::install::UpdateCheck {
        self.update_check.clone().unwrap_or_default()
    }

    fn app_update(&mut self) -> Option<oc_device::install::AppUpdate> {
        self.app_update.clone()
    }

    fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }

    fn set_settings(&mut self, s: DeviceSettings) {
        let locked = self.profiles.get(self.active).is_some_and(|p| p.locked);
        self.settings = crate::model::settings::guard_locked(&self.settings, s, locked);
    }

    fn console(&mut self, command: &str) -> String {
        let mut words = command.split_whitespace();
        match words.next().unwrap_or("") {
            "help" => "help · status · ver · clip · therm · peek <adr>".into(),
            "status" => format!("mode=RUN fs=48000 bits=24 mute={} faults=0", u8::from(self.muted)),
            "ver" => "oc-dsp 0.9.3 · oc-usb 1.2.0".into(),
            "clip" => format!("ampA={} ampB={}", self.telemetry.clips.0, self.telemetry.clips.1),
            "therm" => format!("thermal_trip={}", u8::from(self.telemetry.thermal_trip)),
            "peek" => {
                let arg = words.next().unwrap_or("0").trim_start_matches("0x");
                let base = u32::from_str_radix(arg, 16).unwrap_or(0);
                let mut s = base;
                (0..2)
                    .map(|row| {
                        let bytes: Vec<String> = (0..16)
                            .map(|_| {
                                s = (s.wrapping_mul(1_103_515_245).wrapping_add(12_345)) & 0x7fff_ffff;
                                format!("{:02x}", (s >> 16) & 0xff)
                            })
                            .collect();
                        format!("{:06X}  {}", base + row * 16, bytes.join(" "))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            _ => "unknown command".into(),
        }
    }

    fn begin_update(&mut self, part: FirmwarePart) {
        self.updates[Self::slot(part)] = Some(self.now);
    }

    fn update_progress(&self, part: FirmwarePart) -> Option<UpdateProgress> {
        let start = self.updates[Self::slot(part)]?;
        let percent = (((self.now - start) / UPDATE_S * 25.0).floor() * 4.0).clamp(0.0, 100.0) as u8;
        let step = match (percent / 20).min(4) {
            1 => match part {
                FirmwarePart::Usb => "fw.sim.dfu",
                FirmwarePart::Dsp => "fw.sim.dsp_stop",
            },
            i => UPDATE_STEPS[usize::from(i)],
        };
        Some(UpdateProgress { step: crate::i18n::tr(step), percent })
    }

    fn last_backup(&self) -> String {
        self.backup_date.clone()
    }

    fn backup(&mut self) {}

    fn restore_backup(&mut self) {}

    fn restore_factory(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::eq::{Band, BandType, Target};

    #[test]
    fn starts_on_profile_1_with_the_default_profiles() {
        let d = SimDevice::new();
        assert_eq!((d.volume(), d.muted(), d.active_profile()), (42, false, 0));
        assert_eq!(d.profiles()[3].name, "Profil 4");
        assert_eq!(d.status().stream, StreamFormat::Surround51);
        let mut d = d;
        d.set_mode(DeviceMode::Hq21);
        assert_eq!((d.mode(), d.status().stream), (Some(DeviceMode::Hq21), StreamFormat::Hq21));
    }

    #[test]
    fn profiles_2_to_4_are_editable_profile_1_is_locked() {
        let mut d = SimDevice::new();
        let mut b = ProfileBands::flat();
        b.get_mut(Target::Sub).push(Band::new(BandType::Peak, 50.0, 1.0, 1.0));
        assert!(d.store_profile(1, &b));
        assert_eq!(d.profiles()[1].bands, b);
        d.rename_profile(1, "X");
        assert_eq!(d.profiles()[1].name, "X");
        assert!(!d.store_profile(0, &b));
        d.rename_profile(0, "X");
        d.set_profile_hue(0, 5);
        assert_eq!((d.profiles()[0].name.as_str(), d.profiles()[0].hue), ("Bose d'origine", 0));
    }

    #[test]
    fn rename_trims_and_limits_length() {
        let mut d = SimDevice::new();
        d.rename_profile(1, "   ");
        assert_eq!(d.profiles()[1].name, "Profil 2");
        d.rename_profile(1, "  Un nom vraiment beaucoup trop long  ");
        assert_eq!(d.profiles()[1].name.chars().count(), NAME_MAX);
    }

    #[test]
    fn volume_is_clamped_and_telemetry_stays_in_range() {
        let mut d = SimDevice::new();
        d.set_volume(250);
        assert_eq!(d.volume(), 100);
        for i in 0..200 {
            d.tick(f64::from(i));
            let t = d.telemetry();
            assert!((24.0..=62.0).contains(&t.dsp_load) && (13.38..13.44).contains(&t.supply));
        }
    }

    #[test]
    fn firmware_update_progresses_then_ends() {
        let mut d = SimDevice::new();
        d.tick(5.0);
        d.begin_update(FirmwarePart::Dsp);
        d.tick(5.0 + 0.6);
        let p = d.update_progress(FirmwarePart::Dsp).unwrap();
        assert_eq!(p.percent, 20);
        assert!(crate::i18n::Lang::ALL.iter().any(|l| crate::i18n::lookup(*l, "fw.sim.dsp_stop") == Some(p.step)));
        assert!(d.update_progress(FirmwarePart::Usb).is_none());
        d.tick(9.0);
        assert!(d.update_progress(FirmwarePart::Dsp).is_none());
    }

    #[test]
    fn console_answers() {
        let mut d = SimDevice::new();
        assert!(d.console("ver").starts_with("oc-dsp"));
        assert_eq!(d.console("clip"), "ampA=3 ampB=0");
        assert_eq!(d.console("peek 0x0400").lines().count(), 2);
        assert_eq!(d.console("nope"), "unknown command");
    }
}
