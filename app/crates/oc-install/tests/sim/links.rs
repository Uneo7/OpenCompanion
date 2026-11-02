use std::time::Duration;

use oc_install::{Host, Plan, UiStep};
use oc_protocol::oc_hid::{cmd, status, OcLink};
use oc_protocol::{Error, Link, Result};

use super::tas::TasRun;
use super::Shared;

fn lost(c: u8) -> Error {
    Error::Timeout(c as char)
}

pub struct BoseLink(pub Shared);

impl Link for BoseLink {
    fn transact(&self, c: u8, data: &[u8], _t: Duration) -> Result<(u8, Vec<u8>)> {
        let mut s = self.0.borrow_mut();
        if s.tas.run != TasRun::Bose {
            return Err(Error::NotFound);
        }
        let fault = s.gate().map_err(|_| lost(c))?;
        let reply = match c {
            b't' | b'T' => s.bose_eeprom(c, data),
            b'E' => match s.dsp_e(data) {
                Ok(r) => (b'E', r),
                Err(()) => (b'E', b"ERROR1".to_vec()),
            },
            _ => (c, vec![]),
        };
        s.finish(fault).map_err(|_| lost(c))?;
        Ok(reply)
    }

    fn delay(&self, _d: Duration) {}
}

pub struct OcSim(pub Shared);

impl OcLink for OcSim {
    fn request(&self, c: u8, p: &[u8], _t: Duration) -> Result<(u8, Vec<u8>)> {
        let mut s = self.0.borrow_mut();
        let TasRun::Oc(i) = s.tas.run else { return Err(Error::NotFound) };
        let fault = s.gate().map_err(|_| lost(c))?;
        let reply = match c {
            cmd::INFO => {
                let fw = s.known.tas_oc[i].image.header.firmware_version;
                (status::OK, vec![1, 0, fw, 0, 0, 0x10, (s.tas.session as u8) << 3, 0, 0, 0, 0])
            }
            cmd::DSP => {
                if p.len() < 3 {
                    (status::BAD_LEN, vec![])
                } else {
                    match s.dsp_e(&p[2..]) {
                        Ok(r) => (status::OK, [&[p[0]][..], &r].concat()),
                        Err(()) => (0x11, vec![]),
                    }
                }
            }
            cmd::EE_READ | cmd::EE_BEGIN | cmd::EE_WRITE | cmd::EE_COMMIT | cmd::EE_END => s.oc_eeprom(c, p),
            cmd::REBOOT if p != oc_protocol::oc_hid::key(cmd::REBOOT) => (status::BAD_KEY, vec![]),
            cmd::REBOOT => (status::OK, vec![]),
            _ => (status::UNKNOWN, vec![]),
        };
        s.finish(fault).map_err(|_| lost(c))?;
        if c == cmd::REBOOT && reply.0 == status::OK {
            // The TAS restarts once the reply is collected: the ROM boots U6002 again (the DSP keeps running).
            let k = s.known.clone();
            s.tas.boot(&k);
        }
        Ok(reply)
    }

    fn delay(&self, _d: Duration) {}
}

pub struct TestHost {
    pub speaker: Shared,
    pub answer: bool,
    pub log: Vec<String>,
    pub plans: Vec<Plan>,
    /// Consecutive repeats merged.
    pub steps: Vec<UiStep>,
}

impl TestHost {
    pub fn new(speaker: &Shared, answer: bool) -> Self {
        Self { speaker: speaker.clone(), answer, log: Vec::new(), plans: Vec::new(), steps: Vec::new() }
    }
}

impl Host for TestHost {
    fn log(&mut self, line: &str) {
        self.log.push(line.to_string());
    }

    fn confirm(&mut self, plan: &Plan) -> bool {
        self.speaker.borrow_mut().confirms += 1;
        self.plans.push(plan.clone());
        self.answer
    }

    fn step(&mut self, step: UiStep) {
        if self.steps.last() != Some(&step) {
            self.steps.push(step);
        }
    }

    fn keep_awake(&mut self, on: bool) {
        self.speaker.borrow_mut().awake = on;
    }
}
