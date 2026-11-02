//! Every run restarts from facts read on the device, so any interruption resumes. State machine (M1..M5)
//! and power-loss analysis: `firmware/dsp/boot/docs/UPDATE.md`.

pub(crate) mod bundle;
pub(crate) mod critical;
pub(crate) mod enter;
mod install;
mod plan;
mod runtime;
pub(crate) mod steps;
pub(crate) mod survey;
mod updater;

pub use bundle::DspBundle;
pub use enter::{loader_digests, Loaders, StockRef, STOCK_LOADER_DIGESTS};
pub use install::{install, DspOptions, DspReport};
pub use plan::{plan, DspPlan, Step};
pub use runtime::{detect, reason_text, DspRuntime};
pub use survey::{survey, Boot0State, Survey};
pub use updater::{Reply, Updater, MAX_EE_CRC};
