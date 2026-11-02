//! The two string tables, embedded from `oc-app/i18n/{fr,en}.json` (the only source of UI text).

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

const FR: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/i18n/fr.json"));
const EN: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/i18n/en.json"));

/// UI language. French is the default and the fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Fr,
    En,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::Fr, Lang::En];

    pub fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| code.trim().eq_ignore_ascii_case(l.code()))
    }

    pub(super) fn index(self) -> usize {
        self as usize
    }
}

pub(super) type Table = HashMap<String, String>;

fn parse(name: &str, json: &str) -> Table {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("oc-app/i18n/{name}.json is not a flat string map: {e}"))
}

pub(super) fn tables() -> &'static [Table; 2] {
    static T: OnceLock<[Table; 2]> = OnceLock::new();
    T.get_or_init(|| [parse("fr", FR), parse("en", EN)])
}
