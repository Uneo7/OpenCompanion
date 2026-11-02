//! Dynamic stages are taken at their small-signal (linear) response: clippers, compressors and limiters at
//! unity, the loudness at its largest boost for [`REF_VOLUME_DB`], the spatial control at its steady state
//! for decorrelated stereo. Validated against `re/dsp/stock_sim.py` (`tests.rs`).

mod complex;
mod conditions;
mod curves;
pub(crate) mod graph;
pub(crate) mod image;
mod neutral;
mod source;
mod stock;
#[cfg(test)]
mod tests;

pub use conditions::{loudness_boost_db, Conditions, SoundBase, REF_VOLUME_DB, STEREO_ARRAY, STEREO_DIRECT};
pub use curves::{BaseResponse, FS, OUTPUTS};
pub use stock::StockGraph;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResponseError {
    #[error("no stock DSP backup")]
    NoBackup,
    #[error("stock DSP image: {0}")]
    Image(String),
    #[error("stock graph: {0}")]
    Graph(String),
}
