#![allow(dead_code)]

pub mod dsp;
pub mod fixture;
pub mod links;
pub mod tas;

use std::cell::RefCell;
use std::rc::Rc;

use oc_install::dsp::DspBundle;
use oc_install::tas::TasTarget;

pub use links::{BoseLink, OcSim, TestHost};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    /// The host stops (app crash, cable pulled and replugged): the speaker keeps running.
    HostCrash,
    /// Power lost before the transaction reaches the device.
    PowerBefore,
    /// Power lost right after the device executed it (reply lost).
    PowerAfter,
    /// Power lost while the device executes it (EEPROM write cycle interrupted).
    PowerMid,
}

pub const ALL_FAULTS: [FaultKind; 4] = [FaultKind::HostCrash, FaultKind::PowerBefore, FaultKind::PowerAfter, FaultKind::PowerMid];

#[derive(Debug, Clone, Copy)]
pub struct Fault {
    pub at: usize,
    pub kind: FaultKind,
}

pub struct Known {
    pub dsp: DspBundle,
    pub tas_bose: Vec<u8>,
    pub tas_oc: Vec<TasTarget>,
}

pub struct Speaker {
    pub tas: tas::Tas,
    pub dsp: dsp::Dsp,
    pub known: Rc<Known>,
    pub ops: usize,
    pub fault: Option<Fault>,
    /// Set by a fault until [`Speaker::reconnect`].
    pub link_down: bool,
    /// True while the device executes the faulty transaction in "mid" mode: EEPROM writes are corrupted.
    pub mid: bool,
    pub awake: bool,
    /// Transaction indices of each EE_WRITE_BOOT0, with the keep-awake state at that time.
    pub boot0_ops: Vec<(usize, bool)>,
    pub tas_header_ops: Vec<usize>,
    pub ee_writes: usize,
    pub confirms: usize,
}

pub type Shared = Rc<RefCell<Speaker>>;

impl Speaker {
    pub fn new(known: Rc<Known>) -> Shared {
        let mut s = Speaker {
            tas: tas::Tas::new(known.tas_bose.clone()),
            dsp: dsp::Dsp::new(known.dsp.stock.clone()),
            known,
            ops: 0,
            fault: None,
            link_down: false,
            mid: false,
            awake: false,
            boot0_ops: Vec::new(),
            tas_header_ops: Vec::new(),
            ee_writes: 0,
            confirms: 0,
        };
        s.power_cycle();
        Rc::new(RefCell::new(s))
    }

    pub fn power_cycle(&mut self) {
        let known = self.known.clone();
        self.tas.boot(&known);
        self.dsp.boot(&known.dsp);
    }

    pub fn reconnect(&mut self) {
        self.link_down = false;
        self.fault = None;
        self.mid = false;
    }

    /// Called at the start of every transaction. `Err` = the host sees a transport error.
    /// `Ok(Some(kind))` = execute, then apply the fault (see [`Speaker::finish`]).
    pub fn gate(&mut self) -> Result<Option<FaultKind>, ()> {
        if self.link_down {
            return Err(());
        }
        let n = self.ops;
        self.ops += 1;
        match self.fault {
            Some(f) if f.at == n => {
                self.fault = None;
                match f.kind {
                    FaultKind::HostCrash => {
                        self.link_down = true;
                        Err(())
                    }
                    FaultKind::PowerBefore => {
                        self.power_cycle();
                        self.link_down = true;
                        Err(())
                    }
                    k => {
                        self.mid = k == FaultKind::PowerMid;
                        Ok(Some(k))
                    }
                }
            }
            _ => Ok(None),
        }
    }

    /// After executing a transaction that carried a power fault: power cycle, the reply is lost.
    pub fn finish(&mut self, fault: Option<FaultKind>) -> Result<(), ()> {
        if fault.is_some() {
            self.mid = false;
            self.power_cycle();
            self.link_down = true;
            return Err(());
        }
        Ok(())
    }

    pub fn maybe_corrupt(&self, data: &[u8]) -> Vec<u8> {
        if self.mid {
            data.iter().map(|b| b ^ 0xA5).collect()
        } else {
            data.to_vec()
        }
    }
}
