use std::time::Instant;

use super::Session;
use crate::console::{PROTO_SUB_LEVEL, SLOTS};
use crate::worker::messages::Event;

const DSP_ERRORS_MAX: u32 = 4;

#[derive(Clone, Copy)]
enum Slow {
    Pod,
    Protect,
    Sys,
    Bass,
    Route,
}

const SLOW: [Slow; 5] = [Slow::Pod, Slow::Protect, Slow::Sys, Slow::Bass, Slow::Route];

const PROTO_ROUTE21: u8 = 3;

impl Session {
    pub(super) fn poll(&mut self, now: Instant) {
        if self.timers.usb.due(now) {
            self.poll_usb();
        }
        if self.timers.recheck.due(now) {
            self.recheck();
        }
        if !self.full {
            return;
        }
        if self.timers.profile.due(now) {
            self.poll_profile();
        }
        if self.timers.meters.due(now) {
            self.poll_meters();
        }
        if self.timers.slow.due(now) {
            self.poll_slow();
        }
    }

    fn poll_usb(&mut self) {
        let Some(r) = self.conn.tas_info() else { return };
        if let Some(info) = self.track(r.map_err(Into::into)) {
            if self.last_usb != Some(info) {
                self.last_usb = Some(info);
                self.emit(Event::Usb(info));
                if let Some(link) = self.conn.oc() {
                    let mode = crate::link::mode::get_mode(link, &info);
                    self.emit(Event::Mode(mode));
                }
            }
        }
    }

    /// A stock TAS has no `INFO`: liveness is one EEPROM byte read.
    fn recheck(&mut self) {
        if let crate::link::Conn::Stock(c) = &self.conn {
            // Any failure counts: a handle left over from before a replug may fail without a transport error, and
            // the device must be probed again (fresh enumeration: the same ID may now be OpenCompanion).
            if oc_protocol::tas_eeprom::read_block(c, 0, 1).is_err() {
                self.failures += 1;
            } else {
                self.failures = 0;
            }
        }
        if !self.full {
            self.check_dsp();
        }
    }

    fn poll_profile(&mut self) {
        let r = self.with_dsp(|d| d.op_info());
        if r.is_err() {
            self.dsp_errors += 1;
            if self.dsp_errors >= DSP_ERRORS_MAX {
                // Ask again what runs there; back to the full poll set only if it is still our firmware.
                self.dsp_errors = 0;
                self.full = false;
                self.check_dsp();
            }
        } else {
            self.dsp_errors = 0;
        }
        let Some(info) = self.track(r) else { return };
        let prev = self.last_info.replace(info);
        if prev == Some(info) {
            return;
        }
        self.emit(Event::Info(info));
        let Some(prev) = prev else { return };
        // A load by the pod (mute double press) changes the active slot; a save changes the directory.
        let loaded = prev.active != info.active;
        if loaded || (prev.busy && !info.busy) {
            self.refresh_profiles(loaded);
        }
    }

    pub(super) fn refresh_profiles(&mut self, read_profile: bool) {
        let info = self.with_dsp(|d| d.op_info());
        if let Some(info) = self.track(info) {
            self.last_info = Some(info);
            self.emit(Event::Info(info));
        }
        let slots = self.with_dsp(|d| (0..SLOTS as u8).map(|s| d.op_name(s)).collect::<crate::Result<Vec<_>>>());
        if let Some(slots) = self.track(slots) {
            self.emit(Event::Slots(slots));
        }
        if read_profile {
            let p = self.with_dsp(|d| d.read_profile());
            if let Some(p) = self.track(p) {
                self.mirror = Some(p.clone());
                self.emit(Event::Profile(Box::new(p)));
            }
        }
        let bass = self.with_dsp(|d| d.op_bass());
        if let Some(b) = self.track(bass) {
            self.emit(Event::Bass(b));
        }
        self.poll_route21();
    }

    fn poll_route21(&mut self) {
        if let Some(proto) = self.last_info.map(|i| i.proto).filter(|p| *p >= PROTO_ROUTE21) {
            // Protocol 5 removed OPU: the same reply comes from OPC without argument.
            let r = self.with_dsp(|d| if proto >= PROTO_SUB_LEVEL { d.op_options() } else { d.op_route21() });
            if let Some(r) = self.track(r) {
                self.emit(Event::Route21(r));
            }
        }
    }

    fn poll_meters(&mut self) {
        self.meter_ticks = self.meter_ticks.wrapping_add(1);
        let r = self.with_dsp(|d| -> crate::Result<_> { Ok((d.peak_in()?, d.peak_out()?)) });
        if let Some((pi, po)) = self.track(r) {
            self.meters.peak_in = pi;
            self.meters.peak_out = po;
            self.emit(Event::Meters(self.meters));
        }
    }

    fn poll_slow(&mut self) {
        let which = SLOW[self.slow_turn % SLOW.len()];
        self.slow_turn = self.slow_turn.wrapping_add(1);
        if let Slow::Route = which {
            return self.poll_route21();
        }
        let e = self.with_dsp(|d| -> crate::Result<Event> {
            Ok(match which {
                Slow::Pod => Event::Pod(d.pod()?),
                Slow::Protect => Event::Protect(d.protect()?),
                Slow::Sys => Event::Sys(d.sys()?),
                Slow::Bass => Event::Bass(d.op_bass()?),
                Slow::Route => unreachable!("polled above"),
            })
        });
        if let Some(e) = self.track(e) {
            self.emit(e);
        }
    }
}
