//! Release layout, manifest and cache: `docs/developer/firmware-releases.md`.

pub mod github;
pub mod manifest;
pub mod package;
pub mod store;

pub const REPO: &str = "Uneo7/OpenCompagnon";
pub const API_BASE: &str = "https://api.github.com";
