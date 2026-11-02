use std::time::{Duration, Instant};

use oc_install::dsp::{detect, DspRuntime};
use oc_protocol::oc_hid::{cmd, key};

use super::{Session, MAX_FAILURES};
use crate::error::Error;
use crate::worker::messages::{Event, RestartTarget};

const DSP_BACK: Duration = Duration::from_secs(15);

impl Session {
    pub(super) fn restart_dsp(&mut self) {
        // The DSP resets at once: its reply may never come.
        let _ = self.with_dsp(|d| d.line("RST"));
        let start = Instant::now();
        std::thread::sleep(Duration::from_millis(500));
        let ok = loop {
            if matches!(self.conn.with_dsp(detect), DspRuntime::OcFirmware { .. }) {
                break true;
            }
            if start.elapsed() > DSP_BACK {
                break false;
            }
            std::thread::sleep(Duration::from_millis(250));
        };
        // A fresh start: versions, boot status and profiles are read again.
        self.full = false;
        self.mirror = None;
        self.failures = 0;
        self.check_dsp();
        self.emit(Event::Restarted { target: RestartTarget::Dsp, ok });
    }

    /// The worker finds the speaker again once it is back on the bus.
    pub(super) fn restart_usb(&mut self) -> crate::Result<()> {
        let Some(link) = self.conn.oc() else {
            return Err(Error::Unsupported("the USB controller does not run the OpenCompanion firmware".into()));
        };
        // The controller may reset before its reply: a missing reply is expected.
        let _ = link.call(cmd::REBOOT, &key(cmd::REBOOT), Duration::from_millis(500));
        self.failures = MAX_FAILURES;
        Ok(())
    }
}
