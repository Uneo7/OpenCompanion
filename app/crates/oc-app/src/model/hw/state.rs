use oc_device::console::{BassState, BootStatus, Meters, OpInfo, PodStatus, ProtectStatus, Route21, SlotEntry, SysStatus, Versions};
use oc_device::install::versions::{Running, RunningVersions};
use oc_device::install::Phase;
use oc_device::link::{DeviceMode, Presence, StreamFormat, TasInfo};
use oc_device::worker::{DspState, Event};
use oc_device::worker::DspRuntime;

use crate::model::device::{Connection, LinkState, Status, Telemetry};

#[derive(Debug, Default)]
pub struct HwState {
    pub presence: Option<Presence>,
    pub dsp: Option<DspRuntime>,
    pub versions: Option<Versions>,
    pub tas_version: Option<String>,
    pub boot: Option<BootStatus>,
    pub usb: Option<TasInfo>,
    pub slots: Vec<SlotEntry>,
    pub info: Option<OpInfo>,
    pub bass: Option<BassState>,
    /// `OPU` reply (DSP protocol 3): carries the device options from protocol 4.
    pub route21: Option<Route21>,
    pub mode: Option<DeviceMode>,
    pub meters: Meters,
    pub pod: Option<PodStatus>,
    pub protect: Option<ProtectStatus>,
    pub sys: Option<SysStatus>,
    /// Clip counters at the last "reset" (the speaker counts since its start).
    pub clip_base: (u32, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Changed {
    Nothing,
    Profiles,
    Link,
    Other,
}

impl HwState {
    /// Records `e`; events the state does not keep are returned for the caller.
    pub fn apply(&mut self, e: Event) -> Result<Changed, Event> {
        Ok(match e {
            Event::Presence(p) => {
                if !p.is_connected() {
                    // Everything read from the speaker is stale.
                    *self = Self { clip_base: self.clip_base, ..Self::default() };
                }
                self.presence = Some(p);
                Changed::Link
            }
            Event::Dsp(DspState::Runtime(rt)) => {
                self.dsp = Some(rt);
                Changed::Link
            }
            Event::Dsp(DspState::Unknown) => {
                self.dsp = None;
                Changed::Link
            }
            Event::Versions(v) => {
                self.versions = Some(v);
                Changed::Other
            }
            Event::TasVersion(v) => {
                self.tas_version = Some(v);
                Changed::Other
            }
            Event::Boot(b) => {
                self.boot = Some(b);
                Changed::Other
            }
            Event::Usb(u) => {
                self.usb = Some(u);
                Changed::Other
            }
            Event::Slots(s) => {
                self.slots = s;
                Changed::Profiles
            }
            Event::Info(i) => {
                let active_changed = self.info.map(|o| o.active) != Some(i.active);
                self.info = Some(i);
                if active_changed {
                    Changed::Profiles
                } else {
                    Changed::Nothing
                }
            }
            Event::Bass(b) => {
                self.bass = Some(b);
                Changed::Other
            }
            Event::Mode(m) => {
                self.mode = m;
                Changed::Other
            }
            Event::Route21(r) => {
                self.route21 = Some(r);
                Changed::Other
            }
            Event::Meters(m) => {
                self.meters = m;
                Changed::Nothing
            }
            Event::Pod(p) => {
                self.pod = Some(p);
                Changed::Nothing
            }
            Event::Protect(p) => {
                self.protect = Some(p);
                Changed::Nothing
            }
            Event::Sys(s) => {
                self.sys = Some(s);
                Changed::Nothing
            }
            other @ (Event::Profile { .. } | Event::Console { .. } | Event::Failed { .. } | Event::Provisioned(_) | Event::Restarted { .. }) => {
                return Err(other)
            }
        })
    }

    /// What runs on each chip as the worker sees it (the installer's fallback when its own reads fail).
    pub fn running_versions(&self) -> RunningVersions {
        let dsp = match &self.dsp {
            Some(DspRuntime::Stock { .. }) => Running::Stock,
            Some(DspRuntime::OcFirmware { version }) => Running::Ours(Some(version.clone())),
            Some(DspRuntime::Silent(e)) if oc_device::install::versions::garbled(e) => Running::ProbablyStock,
            Some(DspRuntime::Silent(_)) | None => Running::Unknown,
            Some(other) => Running::Other(other.to_string()),
        };
        let usb = match &self.presence {
            Some(Presence::Stock) => Running::Stock,
            Some(Presence::OpenCompanion) => Running::Ours(self.tas_version.clone()),
            _ => Running::Unknown,
        };
        RunningVersions { dsp, usb }
    }

    pub fn present(&self) -> bool {
        self.presence.as_ref().is_some_and(Presence::is_connected)
    }

    pub fn ready(&self) -> bool {
        self.presence.as_ref().is_some_and(Presence::is_connected) && matches!(self.dsp, Some(DspRuntime::OcFirmware { .. }))
    }

    pub fn connection(&self, installer: Phase) -> Connection {
        if matches!(installer, Phase::Running | Phase::AwaitingConfirmation) {
            return Connection::Installing;
        }
        match &self.presence {
            None | Some(Presence::Absent) => Connection::Absent,
            Some(Presence::Stock) => Connection::Stock,
            Some(Presence::RomDfu { .. }) => Connection::Recovery,
            Some(Presence::Inaccessible(e)) => Connection::Inaccessible(e.clone()),
            Some(Presence::OpenCompanion) => match &self.dsp {
                Some(DspRuntime::OcFirmware { .. }) => Connection::Ready,
                Some(rt) => Connection::DspNotReady(rt.to_string()),
                None => Connection::Absent,
            },
        }
    }

    pub fn status(&self) -> Status {
        let usb = self.usb;
        let firmware_usb = match (&self.tas_version, &self.presence) {
            (Some(v), _) => crate::model::format::display_version(v).to_owned(),
            (None, Some(Presence::Stock)) => match &self.versions {
                Some(v) => format!("Bose {}", v.tas),
                None => "Bose".into(),
            },
            _ => "-".into(),
        };
        let firmware_dsp = match (&self.dsp, &self.versions) {
            (Some(DspRuntime::Stock { .. }), _) => "Bose".into(),
            (Some(DspRuntime::OcFirmware { version }), _) => crate::model::format::display_version(version).to_owned(),
            (Some(rt), _) => rt.to_string(),
            (None, Some(v)) => v.dsp.clone(),
            (None, None) => "-".into(),
        };
        let knob = self.bass.map_or(0, |b| b.knob);
        Status {
            link: if self.ready() { LinkState::Connected } else { LinkState::Reconnecting },
            stream: usb.map_or(StreamFormat::Idle, |u| u.stream_format()),
            firmware_usb,
            firmware_dsp,
            // The USB controller sees the pod (its `INFO` flags), not the DSP.
            pod_present: usb.is_some_and(|u| u.pod_present),
            pod_knob_ok: usb.is_some_and(|u| u.knob_valid),
            // TODO(ui): the view shows an integer; the knob scale is −1667..+1667 (×1000): shown as −10..+10.
            pod_bass: (f32::from(knob) * 10.0 / 1667.0).round() as i8,
            // TODO(firmware): the headphone jack shares the pod mute line; nothing reports it on its own.
            headphones: false,
        }
    }

    pub fn telemetry(&self) -> Telemetry {
        let p = self.protect.unwrap_or_default();
        Telemetry {
            dsp_load: self.info.map_or(0.0, |i| f32::from(i.cpu_pct)),
            supply: p.vraw_mv as f32 / 1000.0,
            thermal_trip: p.hot.0 || p.hot.1,
            clips: (p.cclip.saturating_sub(self.clip_base.0), p.bsclip.saturating_sub(self.clip_base.1)),
            faults: u32::from(p.fault),
        }
    }

    pub fn reset_clips(&mut self) {
        let p = self.protect.unwrap_or_default();
        self.clip_base = (p.cclip, p.bsclip);
    }

    pub fn boot_line(&self) -> Option<String> {
        let b = self.boot?;
        let result = match b.result {
            0 => "no job",
            1 => "busy",
            2 => "OK",
            3 => "EEPROM I/O error",
            4 => "verify error",
            5 => "refused (stock EEPROM)",
            _ => "unknown",
        };
        let start = if b.cold { "cold boot" } else { "RAM load" };
        Some(format!("{start} · trial: {} · last job: {result}", b.trial_text()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_and_status_follow_the_events() {
        let mut s = HwState::default();
        assert_eq!(s.connection(Phase::Confirm), Connection::Absent);
        s.apply(Event::Presence(Presence::Stock)).unwrap();
        s.apply(Event::Dsp(DspState::Runtime(DspRuntime::Stock { svm: "D01.01.01U01.01.03".into() }))).unwrap();
        s.apply(Event::Versions(Versions { dsp: "01.01.01".into(), tas: "01.01.03".into() })).unwrap();
        assert_eq!(s.connection(Phase::Confirm), Connection::Stock);
        assert_eq!((s.status().firmware_usb.as_str(), s.status().firmware_dsp.as_str()), ("Bose 01.01.03", "Bose"));
        assert_eq!(s.status().link, LinkState::Reconnecting);
        s.apply(Event::Presence(Presence::OpenCompanion)).unwrap();
        s.apply(Event::Dsp(DspState::Runtime(DspRuntime::OcFirmware { version: "OC0.0.1".into() }))).unwrap();
        s.apply(Event::TasVersion("0.2.3".into())).unwrap();
        assert_eq!(s.connection(Phase::Done), Connection::Ready);
        assert_eq!(s.connection(Phase::Running), Connection::Installing);
        let st = s.status();
        assert_eq!((st.link, st.firmware_usb.as_str(), st.firmware_dsp.as_str()), (LinkState::Connected, "0.2.3", "0.0.1"));
        s.apply(Event::Presence(Presence::Absent)).unwrap();
        assert!(s.dsp.is_none() && s.tas_version.is_none());
        assert!(s.apply(Event::Failed { what: "x".into(), error: "y".into() }).is_err());
    }

    #[test]
    fn the_pod_is_seen_by_the_usb_controller() {
        let info = |flags0: u8, flags1: u8| {
            let mut p = vec![3, 0, 5, 0, 0x00, flags0, flags1, 1, 0, 0, 0];
            p.resize(23, 0);
            oc_device::link::TasInfo::parse(&p).unwrap()
        };
        let mut s = HwState::default();
        s.apply(Event::Pod(PodStatus { present: true, ..Default::default() })).unwrap();
        assert!(!s.status().pod_present, "never from the DSP");
        s.apply(Event::Usb(info(0x40, 0x02))).unwrap();
        assert!(s.status().pod_present && s.status().pod_knob_ok);
        s.apply(Event::Usb(info(0x40, 0x00))).unwrap();
        assert!(s.status().pod_present && !s.status().pod_knob_ok);
        s.apply(Event::Usb(info(0x00, 0x02))).unwrap();
        assert!(!s.status().pod_present);
    }

    #[test]
    fn clip_counters_reset_to_a_baseline() {
        let mut s = HwState::default();
        s.apply(Event::Protect(ProtectStatus { cclip: 5, bsclip: 2, ..Default::default() })).unwrap();
        assert_eq!(s.telemetry().clips, (5, 2));
        s.reset_clips();
        s.apply(Event::Protect(ProtectStatus { cclip: 7, bsclip: 2, ..Default::default() })).unwrap();
        assert_eq!(s.telemetry().clips, (2, 0));
    }
}
