mod export;
mod import;
pub mod schema;

pub use export::{num, to_json, Form, Options};
pub use import::from_json;

use super::model::Base;
use super::BaseError;

pub const EXTENSION: &str = "ocbase.json";

pub fn export(b: &Base, o: Options) -> String {
    let mut s = serde_json::to_string_pretty(&to_json(b, o)).expect("a base serializes");
    s.push('\n');
    s
}

pub fn import(text: &str) -> Result<Base, BaseError> {
    let j: schema::FileJson = serde_json::from_str(text).map_err(|e| BaseError::Json(e.to_string()))?;
    from_json(&j)
}
