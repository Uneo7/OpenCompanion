//! Only folders marked `.verified` are ever used.

use std::io::Read;
use std::path::{Path, PathBuf};

use super::manifest::{sha256_hex, Manifest, MANIFEST_NAME};

const VERIFIED: &str = ".verified";

fn folder(tag: &str) -> String {
    tag.chars().map(|c| if c.is_ascii_alphanumeric() || "._-+".contains(c) { c } else { '_' }).collect()
}

pub fn cached(root: &Path, tag: &str) -> Option<PathBuf> {
    let d = root.join(folder(tag));
    d.join(VERIFIED).is_file().then_some(d)
}

pub fn latest(root: &Path) -> Option<(String, PathBuf, Manifest)> {
    let mut best: Option<(std::time::SystemTime, String, PathBuf, Manifest)> = None;
    for e in std::fs::read_dir(root).ok()?.flatten() {
        let dir = e.path();
        let Ok(meta) = std::fs::metadata(dir.join(VERIFIED)) else { continue };
        let Ok(m) = std::fs::read_to_string(dir.join(MANIFEST_NAME)).map_err(|e| e.to_string()).and_then(|j| Manifest::parse(&j)) else {
            continue;
        };
        let when = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
        if best.as_ref().is_none_or(|b| when > b.0) {
            let tag = std::fs::read_to_string(dir.join(VERIFIED)).unwrap_or_default().trim().to_owned();
            best = Some((when, tag, dir, m));
        }
    }
    best.map(|(_, t, d, m)| (t, d, m))
}

pub fn install(root: &Path, tag: &str, zip: &[u8], zip_sha256: Option<&str>, manifest: &Manifest) -> Result<PathBuf, String> {
    if let Some(want) = zip_sha256 {
        if sha256_hex(zip) != want.to_ascii_lowercase() {
            return Err("the downloaded firmware does not match its SHA-256".into());
        }
    }
    let dest = root.join(folder(tag));
    let tmp = root.join(format!("{}.partial", folder(tag)));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let r = extract(zip, &tmp).and_then(|()| {
        // The manifest inside the zip must be the published one, and every file must match it.
        let inner = std::fs::read_to_string(tmp.join(MANIFEST_NAME)).map_err(|e| format!("{MANIFEST_NAME}: {e}"))?;
        if Manifest::parse(&inner)? != *manifest {
            return Err("the manifest in the zip differs from the published one".into());
        }
        manifest.verify_dir(&tmp)?;
        // The containers are checked too (magic, chip, CRC-32, app version) and opened next to them: the
        // folder then has the layout of `FirmwareSources::from_dir`.
        crate::install::ocfw::expand_release(&tmp, &tmp, env!("CARGO_PKG_VERSION")).map(drop)
    });
    if let Err(e) = r {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    std::fs::write(tmp.join(VERIFIED), tag).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}

/// Extracts the files of the zip (no absolute path, no `..`).
fn extract(zip: &[u8], to: &Path) -> Result<(), String> {
    let mut a = zip::ZipArchive::new(std::io::Cursor::new(zip)).map_err(|e| format!("zip: {e}"))?;
    for i in 0..a.len() {
        let mut f = a.by_index(i).map_err(|e| format!("zip: {e}"))?;
        if f.is_dir() {
            continue;
        }
        let Some(rel) = f.enclosed_name() else { return Err(format!("zip: unsafe path {}", f.name())) };
        let p = to.join(rel);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let mut data = Vec::new();
        f.read_to_end(&mut data).map_err(|e| format!("zip: {e}"))?;
        std::fs::write(&p, data).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::release::manifest::tests::fake_release;
    use crate::install::release::package::zip_dir;

    #[test]
    fn verified_releases_only() {
        let base = std::env::temp_dir().join(format!("oc-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let src = base.join("src");
        let m = fake_release(&src);
        std::fs::write(src.join(MANIFEST_NAME), m.to_json()).unwrap();
        let zip = zip_dir(&src).unwrap();
        let root = base.join("cache");
        // Wrong zip digest: refused, nothing kept.
        assert!(install(&root, "v1", &zip, Some(&"0".repeat(64)), &m).is_err());
        assert!(cached(&root, "v1").is_none() && latest(&root).is_none());
        // A manifest that does not match the files: refused.
        let mut other = m.clone();
        other.files[0].sha256 = "1".repeat(64);
        assert!(install(&root, "v1", &zip, None, &other).is_err());
        let dir = install(&root, "v1", &zip, Some(&sha256_hex(&zip)), &m).unwrap();
        assert!(dir.join("boot/updater.out").is_file() && dir.join("oc_tas.dfu").is_file());
        assert_eq!(cached(&root, "v1"), Some(dir.clone()));
        let (tag, d, mm) = latest(&root).unwrap();
        assert_eq!((tag.as_str(), d, mm), ("v1", dir, m));
        let _ = std::fs::remove_dir_all(&base);
    }
}
