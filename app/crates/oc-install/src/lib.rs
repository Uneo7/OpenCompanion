//! Every operation is idempotent and resumable: the state is read back from the EEPROMs at the start of
//! each run, and only what differs from the target is written.

pub mod awake;
pub mod backup;
pub mod dsp;
pub mod paths;
mod error;
mod host;
pub mod restore;
pub mod status;
pub mod tas;
pub mod ui;

pub use error::{Error, Result};
pub use host::{Host, Plan};
pub use ui::UiStep;
