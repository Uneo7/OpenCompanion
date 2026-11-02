//! Same extraction as `re/dsp/gen_bose_graph.py`; every value is kept as the stock float32, so the payload equals
//! `bg_f` bit for bit (golden test in `tests.rs`). A backup not shaped like topology 1 is refused.

use super::model::{Base, BassSum, Coefs, MixPair, Set, Source};
use super::topology::{stock, SetDef, DELAYS, SETS};
use super::BaseError;
use crate::response::graph::{biquad_sets, Kind, Module};
use crate::response::image::Ram;
use crate::response::StockGraph;

/// Stock firmware the addresses of [`super::topology`] belong to.
pub const STOCK_FIRMWARE: &str = "D01.01.01";

fn topo(s: String) -> BaseError {
    BaseError::Topology(s)
}

struct Reader<'a> {
    g: &'a StockGraph,
    ram: &'a Ram,
}

impl Reader<'_> {
    fn module(&self, addr: u32, kind: Kind) -> Result<&Module, BaseError> {
        let m = self.g.module(addr)?;
        if m.kind != kind {
            return Err(topo(format!("0x{addr:08x} is {:?}, expected {kind:?}", m.kind)));
        }
        Ok(m)
    }

    /// `n` float32 targets through the parameter pointer (mixer, gain, sum).
    fn targets(&self, addr: u32, kind: Kind, n: usize) -> Result<Vec<f32>, BaseError> {
        let m = self.module(addr, kind)?;
        let p = self.ram.u(m.params)?;
        (0..n as u32).map(|k| Ok(f32::from_bits(self.ram.u(p + 4 * k)?))).collect()
    }

    fn mixer(&self, addr: u32) -> Result<[f32; 2], BaseError> {
        let m = self.module(addr, Kind::Mixer)?;
        if m.nout != 1 {
            return Err(topo(format!("mixer 0x{addr:08x} has {} outputs", m.nout)));
        }
        let t = self.targets(addr, Kind::Mixer, 2)?;
        Ok([t[0], t[1]])
    }

    fn pair(&self, mods: [u32; 2]) -> Result<MixPair, BaseError> {
        let (l, r) = (self.mixer(mods[0])?, self.mixer(mods[1])?);
        Ok(MixPair { a: [l[0], r[0]], b: [l[1], r[1]] })
    }

    /// Sections active in at least one set of the module (gen_bose_graph.py `keep`).
    fn biquad(&self, def: &SetDef) -> Result<Set, BaseError> {
        let m = self.module(def.stock, Kind::Biquad)?;
        let (sets, chans) = biquad_sets(self.ram, m)?;
        if def.id == super::topology::SetId::EqDirect && chans != stock::DRIVER_EQ_CHANNELS {
            return Err(topo(format!("driver EQ channel sets {chans:?}")));
        }
        let set = sets.get(def.stock_set).ok_or_else(|| topo(format!("0x{:08x} has no set {}", def.stock, def.stock_set)))?;
        let nact = set.sections.len();
        let keep: Vec<usize> = (0..nact).filter(|&k| sets.iter().any(|s| s.sections[k].iter().any(|&c| c != 0.0))).collect();
        if keep.len() != def.nsec {
            return Err(topo(format!("0x{:08x}: {} sections, topology has {}", def.stock, keep.len(), def.nsec)));
        }
        // f64 values of the response model are exact images of the stock float32 words.
        let f = |x: f64| x as f32;
        let sections = keep.iter().map(|&k| set.sections[k].map(f) as Coefs).collect();
        Ok(Set { gain: f(set.g), sections })
    }

    fn delay(&self, addr: u32) -> Result<u8, BaseError> {
        let m = self.module(addr, Kind::Delay)?;
        let d = self.ram.u(m.params + 4)?;
        u8::try_from(d).ok().filter(|&d| d <= super::topology::DELAY_MAX).ok_or_else(|| topo(format!("delay 0x{addr:08x} = {d}")))
    }

    /// The fixed [F] unity mixers must be (1, 1) for topology 1 to be exact.
    fn check_fixed(&self) -> Result<(), BaseError> {
        for a in stock::UNITY_MIXERS {
            if self.mixer(a)? != [1.0, 1.0] {
                return Err(topo(format!("fixed mixer 0x{a:08x} is not (1, 1)")));
            }
        }
        Ok(())
    }
}

pub fn from_stock(g: &StockGraph, backup_crc32: Option<u32>) -> Result<Base, BaseError> {
    let r = Reader { g, ram: g.ram() };
    r.check_fixed()?;
    let mut b = Base::blank("Bose d'origine");
    b.source = Some(Source { kind: "stock-backup".into(), firmware: Some(STOCK_FIRMWARE.into()), backup_crc32 });
    b.aux_mix = r.pair(stock::AUX_MIX)?;
    b.input_gain.copy_from_slice(&r.targets(stock::INPUT_GAIN, Kind::Gain, 6)?);
    b.centre_mix = r.pair(stock::CENTRE_MIX)?;
    b.surround_mix = r.pair(stock::SURROUND_MIX)?;
    let w = r.targets(stock::BASS_SUM, Kind::SumDiff, 7)?;
    b.bass_sum = BassSum { direct: [w[0], w[1]], array: [w[2], w[3]], surround: [w[4], w[5]], lfe: w[6] };
    b.sets = SETS.iter().map(|d| r.biquad(d)).collect::<Result<_, _>>()?;
    for (k, (_, _, _, a)) in DELAYS.iter().enumerate() {
        b.delays[k] = r.delay(*a)?;
    }
    Ok(b)
}

pub fn from_stock_dump(dsp: &[u8]) -> Result<Base, BaseError> {
    let g = StockGraph::from_stock_dump(dsp)?;
    from_stock(&g, Some(crc32fast::hash(dsp)))
}

/// From the oldest verified stock backup under `root` (`oc_install::backup::default_root()`), as the
/// response curves do.
pub fn from_backups(root: &std::path::Path) -> Result<Base, BaseError> {
    use crate::response::ResponseError;
    let entry = oc_install::backup::stock_backup_status(root).ok_or(ResponseError::NoBackup)?;
    let files = oc_install::backup::load(&entry.dir).map_err(|e| ResponseError::Image(e.to_string()))?;
    from_stock_dump(&files.dsp.ok_or(ResponseError::NoBackup)?)
}
