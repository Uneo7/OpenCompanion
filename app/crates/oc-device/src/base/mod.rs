pub mod coefs;
pub mod convert;
pub mod json;
pub mod model;
pub mod payload;
pub mod topology;
pub mod validate;
#[cfg(test)]
mod tests;

pub use model::{Base, BassSum, Coefs, MixPair, Set, Source};

use crate::response::ResponseError;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum BaseError {
    #[error(transparent)]
    Stock(#[from] ResponseError),
    #[error("stock program does not match topology c5-stock-1: {0}")]
    Topology(String),
    #[error("base file: {0}")]
    Json(String),
    #[error("invalid base: {0}")]
    Invalid(String),
    #[error("base payload: {0}")]
    Payload(String),
}
