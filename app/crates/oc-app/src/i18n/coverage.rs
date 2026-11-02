//! Every i18n key the workspace uses exists in both tables.
//!
//! Literal keys are found by scanning the Rust sources of every crate for string literals that start with a
//! namespace of the tables (`"eq.…"`, `"settings.…"`, …). Keys built at run time are listed by family below.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::{catalog, Lang};
use crate::model::BandType;

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "target") {
                rust_files(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

fn is_key_shaped(s: &str, namespaces: &BTreeSet<&str>) -> bool {
    let Some((ns, rest)) = s.split_once('.') else { return false };
    namespaces.contains(ns)
        && !rest.is_empty()
        && !s.ends_with('.')
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.')
}

/// Key-shaped string literals of the workspace sources, with the file they were found in.
fn literal_keys() -> Vec<(String, PathBuf)> {
    let tables = catalog::tables();
    let namespaces: BTreeSet<&str> = tables.iter().flat_map(|t| t.keys()).filter_map(|k| k.split('.').next()).collect();
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    rust_files(&crates, &mut files);
    assert!(files.len() > 50, "found only {} sources under {}", files.len(), crates.display());
    let mut out = Vec::new();
    for f in files {
        let src = std::fs::read_to_string(&f).unwrap_or_default();
        for line in src.lines() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            // Odd segments of a split on '"' are the insides of string literals.
            for lit in code.split('"').skip(1).step_by(2) {
                if is_key_shaped(lit, &namespaces) {
                    out.push((lit.to_owned(), f.clone()));
                }
            }
        }
    }
    out
}

/// Keys built with `format!` (the families the views and widgets expand at run time).
fn dynamic_keys() -> Vec<String> {
    let mut out = Vec::new();
    for i in 1..=9 {
        out.push(format!("eq.help.g{i}"));
        out.push(format!("eq.help.e{i}"));
    }
    for i in 1..=crate::model::profile::PROFILE_COUNT {
        out.push(format!("profile.default.p{i}"));
    }
    for i in 1..=5 {
        out.push(format!("legal.risk.{i}"));
        out.push(format!("legal.risk.{i}.lead"));
    }
    for k in BandType::ALL {
        let key = crate::widgets::fmt::type_key(k);
        out.push(key.to_owned());
        out.push(format!("{key}.help"));
    }
    out
}

#[test]
fn every_used_key_exists_in_both_tables() {
    let mut missing = Vec::new();
    let used = literal_keys();
    assert!(used.len() > 200, "the scan found only {} keys", used.len());
    let all = used.into_iter().map(|(k, f)| (k, f.display().to_string())).chain(dynamic_keys().into_iter().map(|k| (k, "format!".into())));
    for (key, at) in all {
        for l in Lang::ALL {
            if super::lookup(l, &key).is_none() {
                missing.push(format!("{} {key} ({at})", l.code()));
            }
        }
    }
    missing.sort();
    missing.dedup();
    assert!(missing.is_empty(), "i18n keys missing:\n{}", missing.join("\n"));
}

#[test]
fn both_tables_have_the_same_keys() {
    let missing = super::missing_keys();
    assert!(missing.is_empty(), "keys in one table only: {missing:?}");
}
