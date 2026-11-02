//! Order: R1 header := zeros, body; R2 boot-state page and profiles, stock header last (boot1 stays in rescue
//! throughout); R3 stock boot0 in ONE `EE_WRITE_BOOT0`, the only brick window (~17 ms); R4 boot1 area (0xFF):
//! the stock AIS stream ends below 0xDB00, the ROM never reads further; R5 full verification.
//!
//! boot1 is restored after boot0, not before: our boot0 runs boot1 from 0xDB00, so overwriting boot1 while boot0
//! is still ours would turn every R4 page write into a brick window (73 pages instead of ~17 ms).
//!
//! Guards: the backup must be a stock image (AIS magic + Disable-CRC, no OCI1 header, AIS stream complete below
//! 0xDB00); its last page (never writable) must equal the speaker's, which also tells that the backup comes from
//! this speaker; a stock boot0 with a different image below 0xDB00 is refused (that image boots as is: writing
//! it would open a long brick window); right before R3 the whole 0x0100..0xDAFF range is CRC-checked against
//! the backup. The updater's boot unlock (`EE_UNLOCK_BOOT`) is only sent for R3 and R4.

mod image;
mod plan;
mod run;
mod steps;
mod survey;

pub use image::DspRestoreInputs;
pub use plan::{plan, RestorePlan, RestoreStep};
pub use run::{restore_dsp, DspRestoreOptions, DspRestoreReport};
pub use survey::{survey, Boot0Kind, RestoreSurvey};
