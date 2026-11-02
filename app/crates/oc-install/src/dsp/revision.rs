//! RAM layouts of the stock Bose DSP firmware that the RAM loader can take over. The entry is chosen by the
//! speaker's RAM content (hijack signature and loader-area digests), never by the `SVM` version: a later Bose
//! revision with a known layout is taken over as is, any other is refused before a RAM write.
//!
//! The stub (`firmware/dsp/loader`) is linked at 0x10023000 and only touches hardware once running; what differs
//! between revisions is where the stock firmware keeps its dead `.cinit` table (the loader area, checked before
//! any RAM write) and the address of the output meter's process function (the hijack target, read before any RAM
//! write). Both were derived statically from each revision's U7001 image (AIS sections, `.cinit` records):
//!
//! | | D01.01.01 | D01.01.03 |
//! |---|---|---|
//! | `.cinit` | 0x1001DB40..0x10024E44 | 0x1001D460..0x10024764 (same 723 records, same data but the version string) |
//! | loader area checked | 0x10023000..0x10024E00 (30 samples) | 0x10023000..0x10024760 (24 samples, all in `.cinit`) |
//! | output meter module | 0x10011C4C, flags 3 | 0x10011C4C, flags 3 (same data layout) |
//! | process function | 0x1002CDDC | 0x1002C81C (class code moved by -0x5C0, 108/112 words identical) |
//!
//! The stub writes 0x10023000..0x100243FF, inside `.cinit` in both. On D01.01.03 its stack and handoff record
//! (0x10024800..0x10024E1F, used only after the takeover, when the stock firmware never runs again until the
//! next boot) fall in the section after `.cinit`; nothing is written there through `BWW`. The stock variables
//! the stub's `handoff_capture` reads (0x10006D61, 0x10010100 / 08 / 14) are at the same addresses in both
//! (identical `.cinit` data layout, same code references). Digests are CRC-32s of 16-byte samples, not
//! firmware bytes.

use oc_ramload as rl;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StockRevision {
    /// `D<version>` part of the `SVM` answer.
    pub dsp: &'static str,
    /// The output meter module whose process pointer is redirected to the stub.
    pub module: u32,
    /// Its process pointer in this revision (checked, read-only, before any RAM write).
    pub process: u32,
    /// End of the checked loader area (start: [`rl::LOADER_LO`]); must hold every stub segment.
    pub loader_hi: u32,
    /// CRC-32 of the 16 bytes at `LOADER_LO + 0x100 * i`, from this revision's stock image.
    pub digests: &'static [u32],
}

impl StockRevision {
    pub fn samples(&self) -> impl Iterator<Item = u32> {
        (rl::LOADER_LO..self.loader_hi).step_by(0x100)
    }

    pub fn flags_addr(&self) -> u32 {
        self.module + 16
    }

    pub fn slot_addr(&self) -> u32 {
        self.module + 20
    }
}

/// D01.01.01 (`STOCK_LOADER_DIGESTS`, checked against a real dump by `tests/real_inputs.rs`).
pub const D010101: StockRevision = StockRevision {
    dsp: "D01.01.01",
    module: rl::HIJACK_MODULE,
    process: rl::HIJACK_ORIGINAL,
    loader_hi: rl::LOADER_HI,
    digests: &super::enter::STOCK_LOADER_DIGESTS,
};

/// D01.01.03 (Bose soft update; checked against a real dump by `tests/real_inputs.rs` when `OC_STOCK_D010103`
/// points to it).
pub const D010103: StockRevision = StockRevision {
    dsp: "D01.01.03",
    module: rl::HIJACK_MODULE,
    process: 0x1002_C81C,
    loader_hi: 0x1002_4760,
    digests: &[
        0x45483b64, 0xecbb4b55, 0xabcfc5ef, 0x868e3880, 0x3f008675, 0x6b05b83b, 0x143603d5, 0xe83606ea,
        0xdf1911df, 0x258a7c4e, 0x76ed126a, 0x441308a7, 0x5bbea989, 0x5303d8ed, 0x17bf0a82, 0xbbdc84b6,
        0x7d8ed042, 0xd37f12dd, 0x9327d784, 0x099d5a31, 0xd4a83065, 0xc36a7606, 0xd6c3043f, 0x09e7ef0d,
    ],
};

pub const REVISIONS: [StockRevision; 2] = [D010101, D010103];

/// The table entry named after a stock `SVM` answer (`D01.01.03U01.01.04`, possibly with `\r>`). Informative
/// only (logs, tests): the RAM loader picks its entry by the speaker's RAM content (`enter.rs` `match_layout`).
pub fn for_svm(svm: &str) -> Option<&'static StockRevision> {
    let dsp = svm.trim().split('U').next()?.trim();
    REVISIONS.iter().find(|r| r.dsp == dsp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_by_svm() {
        assert_eq!(for_svm("D01.01.01U01.01.03").unwrap().dsp, "D01.01.01");
        assert_eq!(for_svm("D01.01.03U01.01.04").unwrap().process, 0x1002_C81C);
        assert!(for_svm("D01.01.02U01.01.03").is_none());
        assert!(for_svm("D01.01.031U01.01.04").is_none(), "an exact version, not a prefix");
        assert!(for_svm("").is_none());
    }

    #[test]
    fn tables_are_consistent() {
        for r in REVISIONS {
            assert_eq!(r.samples().count(), r.digests.len(), "{}", r.dsp);
            assert!(r.loader_hi <= rl::LOADER_HI, "{}: the loader area is a part of the stub's", r.dsp);
        }
    }
}
