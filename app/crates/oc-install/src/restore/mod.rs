//! Restore only from the user's own backups: no Bose firmware is shipped with OpenCompanion.
//! Restore the DSP first, then the TAS: the stock TAS firmware still relays to rescue mode and to the updater,
//! so a DSP restore can be resumed after the TAS one too.

pub mod dsp;
pub mod tas;

pub use dsp::{restore_dsp, DspRestoreInputs, DspRestoreOptions, DspRestoreReport, RestorePlan, RestoreStep};
pub use tas::{reboot_tas, restore_tas, stock_tas_target, TasRestoreReport};
