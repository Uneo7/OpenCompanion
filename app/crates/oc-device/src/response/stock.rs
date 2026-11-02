//! Small-signal response of the stock graph as the OpenCompanion "Bose d'origine" program runs it
//! (`firmware/dsp/src/bose/`): the module tree is walked in execution order from the USB gain 0x100106C8 to the
//! band split 0x10011450, as `re/dsp/stock_sim.py` does, and complex spectra are propagated through the wires.
//!
//! Steady state (normal mode "N", pod not muted; `re/dsp/notes/bose_profile.md` section 1):
//! - USB gain 0x100106C8 = 1 here (its stock target, the input margin, is [`StockGraph::usb_level`]);
//! - BADDATA selector 0x10010818 on the direct branch; mode mixers fixed to "N";
//! - loudness: direct gain 0x100103FC = 1, band gain 0x10010430 = `loud_band` (master volume factored out);
//! - bass knob mixer 0x10010938 = (1, 0): OpenCompanion keeps only the all-pass branch (the sub level is a
//!   separate gain after the program);
//! - spatial matrix 0x10010B58 forced to the given direct / array gains.
//!
//! Everything after the band split (band clippers, compressors, limiters) is unity in small signal; the
//! satellites' 320-sample look-ahead delay only changes the phase.

use std::collections::HashMap;

use super::complex::{mix, product, scaled, Spectrum, C64};
use super::graph::{biquad_sets, Kind, Module, FLAG_CONTAINER, FLAG_CONTROL, ROOT};
use super::image::Ram;
use super::ResponseError;

const FIRST: u32 = 0x1001_06C8;
const STOP: u32 = 0x1001_1450;
/// Modules whose first input is an output of the program: the band splits of inner L, inner R, outer L,
/// outer R, then the bass compressor.
const OUT_MODS: [u32; 5] = [0x1001_15C4, 0x1001_18AC, 0x1001_1450, 0x1001_1738, 0x1001_1A54];

const USB_GAIN: u32 = 0x1001_06C8;
const LOUD_DIRECT: u32 = 0x1001_03FC;
const LOUD_BAND: u32 = 0x1001_0430;
const BADDATA_MIX: u32 = 0x1001_0818;
const BASS_KNOB_MIX: u32 = 0x1001_0938;
const SPATIAL_MIX: u32 = 0x1001_0B58;
/// Loudness band-pass 0x100103C0 (also used by the neutral program).
pub const LOUD_BP: u32 = 0x1001_03C0;
/// Mode mixers / gains written by the mode selector, state 4 ("N").
const NORMAL_MODE: [(u32, &[f64]); 5] = [
    (0x1001_0C20, &[1.0; 4]),
    (0x1001_0C54, &[0.0; 8]),
    (0x1001_0C7C, &[1.0; 4]),
    (0x1001_0CF4, &[1.0, 0.0]),
    (0x1001_0D1C, &[1.0, 0.0]),
];

#[derive(Debug, Clone)]
pub struct StockGraph {
    pub(super) ram: Ram,
    root: Module,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Steady {
    /// Loudness band gain relative to the direct gain, 10^(boost / 20) - 1 (0 without loudness).
    pub loud_band: f64,
    pub direct: f64,
    pub array: f64,
}

impl StockGraph {
    pub fn new(ram: Ram) -> Result<Self, ResponseError> {
        let root = Module::read(&ram, ROOT)?;
        if root.flags != FLAG_CONTAINER || root.ins.len() != 8 {
            return Err(ResponseError::Graph("unexpected root module (not the stock firmware D01.01.01?)".into()));
        }
        for a in [FIRST, STOP, LOUD_BP, BADDATA_MIX, BASS_KNOB_MIX, SPATIAL_MIX].iter().chain(OUT_MODS.iter()) {
            if root.find(*a).is_none() {
                return Err(ResponseError::Graph(format!("module 0x{a:08x} not in the graph")));
            }
        }
        Ok(Self { ram, root })
    }

    pub(crate) fn ram(&self) -> &Ram {
        &self.ram
    }

    pub(crate) fn module(&self, addr: u32) -> Result<&Module, ResponseError> {
        self.root.find(addr).ok_or_else(|| ResponseError::Graph(format!("module 0x{addr:08x} not in the graph")))
    }

    /// Stock target of the USB gain 0x100106C8 (the input margin, 0.340408): OpenCompanion feeds the USB
    /// channels into the program at this level (`BG_USB_LEVEL`).
    pub fn usb_level(&self) -> Result<f64, ResponseError> {
        let m = self.module(USB_GAIN)?;
        self.ram.f(self.ram.u(m.params)?)
    }

    /// Responses from root input `src` (0 = USB L, 1 = USB R, ... 7 = AUX R) to the five outputs (inner L,
    /// inner R, outer L, outer R, bass), USB gain excluded, at angular frequencies `w` (rad / sample).
    pub(super) fn responses(&self, src: usize, st: &Steady, w: &[f64]) -> Result<[Spectrum; 5], ResponseError> {
        let mut run = Run { g: self, st, w, wires: HashMap::new(), started: false, done: false };
        for (k, p) in self.root.ins.iter().enumerate() {
            run.wires.insert(*p, vec![C64::new(if k == src { 1.0 } else { 0.0 }, 0.0); w.len()]);
        }
        run.walk(&self.root)?;
        if !run.done {
            return Err(ResponseError::Graph("end of the program not reached".into()));
        }
        let mut out: [Spectrum; 5] = Default::default();
        for (o, a) in out.iter_mut().zip(OUT_MODS) {
            let m = self.module(a)?;
            let p = *m.ins.first().ok_or_else(|| ResponseError::Graph(format!("0x{a:08x} has no input")))?;
            *o = run.get(p);
        }
        Ok(out)
    }
}

struct Run<'a> {
    g: &'a StockGraph,
    st: &'a Steady,
    w: &'a [f64],
    wires: HashMap<u32, Spectrum>,
    started: bool,
    done: bool,
}

fn graph_err(s: String) -> ResponseError {
    ResponseError::Graph(s)
}

impl Run<'_> {
    fn get(&self, p: u32) -> Spectrum {
        self.wires.get(&p).cloned().unwrap_or_else(|| vec![C64::ZERO; self.w.len()])
    }

    fn walk(&mut self, m: &Module) -> Result<(), ResponseError> {
        for c in &m.children {
            if self.done {
                return Ok(());
            }
            if c.addr == FIRST {
                self.started = true;
            }
            if c.addr == STOP {
                self.done = true;
                return Ok(());
            }
            if c.flags == FLAG_CONTAINER {
                self.walk(c)?;
            } else if self.started && c.flags != FLAG_CONTROL {
                self.module(c)?;
            }
        }
        Ok(())
    }

    /// Targets of a gain or mixer: steady-state value, else the RAM values.
    fn targets(&self, m: &Module, n: usize) -> Result<Vec<f64>, ResponseError> {
        Ok(match m.addr {
            USB_GAIN | LOUD_DIRECT => vec![1.0; n],
            LOUD_BAND => vec![self.st.loud_band; n],
            BADDATA_MIX => (0..n).map(|k| if k % 2 == 0 { 1.0 } else { 0.0 }).collect(),
            BASS_KNOB_MIX => vec![1.0, 0.0],
            SPATIAL_MIX => {
                let (d, a) = (self.st.direct, self.st.array);
                vec![d, 0.0, 0.0, d, a, 0.0, 0.0, a, 0.0, 0.0, 0.0, 0.0]
            }
            a => match NORMAL_MODE.iter().find(|(x, _)| *x == a) {
                Some((_, v)) => v.to_vec(),
                None => self.g.ram.floats(self.g.ram.u(m.params)?, n)?,
            },
        })
    }

    fn module(&mut self, m: &Module) -> Result<(), ResponseError> {
        let ins: Vec<Spectrum> = m.ins.iter().map(|p| self.get(*p)).collect();
        let ram = &self.g.ram;
        let at = |t: &[f64], i: usize| t.get(i).copied().unwrap_or(0.0);
        let pairs = |h: usize| {
            if ins.len() < 2 * h {
                Err(graph_err(format!("module 0x{:08x}: {} inputs for {h} outputs", m.addr, ins.len())))
            } else {
                Ok(())
            }
        };
        let outs: Vec<Spectrum> = match m.kind {
            Kind::Biquad => {
                let (sets, chans) = biquad_sets(ram, m)?;
                ins.iter().zip(chans).map(|(x, s)| product(x, &sets[s].response(self.w))).collect()
            }
            Kind::Gain => {
                let t = self.targets(m, m.nin.max(m.nout))?;
                ins.iter().enumerate().map(|(k, x)| scaled(x, at(&t, k))).collect()
            }
            Kind::Mixer => {
                let t = self.targets(m, m.nin)?;
                let h = m.nout;
                pairs(h)?;
                (0..h).map(|j| mix(&ins[j], at(&t, 2 * j), &ins[h + j], at(&t, 2 * j + 1))).collect()
            }
            Kind::Delay => {
                let d = f64::from(ram.u(m.params + 4)?);
                let x = ins.first().cloned().unwrap_or_default();
                vec![x.iter().zip(self.w).map(|(v, w)| *v * C64::delay(w * d)).collect()]
            }
            Kind::SumDiff => {
                let c = ram.floats(ram.u(m.params)?, m.nin)?;
                let mut acc = vec![C64::ZERO; self.w.len()];
                for (x, k) in ins.iter().zip(c) {
                    acc = mix(&acc, 1.0, x, k);
                }
                vec![acc]
            }
            Kind::Router => {
                let tab = ram.u(m.params)?;
                (0..m.nout as u32)
                    .map(|j| {
                        let i = ram.u(tab + 4 * j)? as usize;
                        ins.get(i).cloned().ok_or_else(|| graph_err(format!("router 0x{:08x}: no input {i}", m.addr)))
                    })
                    .collect::<Result<_, _>>()?
            }
            Kind::Adder => {
                let h = m.nout;
                pairs(h)?;
                (0..h).map(|j| mix(&ins[j], 1.0, &ins[h + j], 1.0)).collect()
            }
            Kind::Meter => return Ok(()),
            Kind::Other(p) => return Err(graph_err(format!("module 0x{:08x}: unknown process 0x{p:08x}", m.addr))),
        };
        for (p, y) in m.outs.iter().zip(outs) {
            self.wires.insert(*p, y);
        }
        Ok(())
    }
}
