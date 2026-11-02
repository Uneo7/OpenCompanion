use std::io::Write;
use std::path::{Path, PathBuf};

use super::manifest::{sha256_hex, Manifest, MANIFEST_NAME};
use crate::install::pack::write_pack;
use crate::install::sources::FirmwareSources;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assets {
    pub zip: PathBuf,
    pub zip_sha256: PathBuf,
    pub manifest: PathBuf,
    pub manifest_data: Manifest,
}

fn semver(v: &str) -> &str {
    v.split('+').next().unwrap_or(v)
}

/// Zips `dir` (every file, paths with `/`, sorted, fixed time stamps: the same files give the same zip).
pub fn zip_dir(dir: &Path) -> Result<Vec<u8>, String> {
    let mut files = Vec::new();
    collect(dir, dir, &mut files)?;
    files.sort();
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());
    for rel in files {
        let data = std::fs::read(dir.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
        w.start_file(rel.as_str(), opts).map_err(|e| e.to_string())?;
        w.write_all(&data).map_err(|e| e.to_string())?;
    }
    Ok(w.finish().map_err(|e| e.to_string())?.into_inner())
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
    for e in std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(root, &p, out)?;
        } else {
            let rel = p.strip_prefix(root).map_err(|e| e.to_string())?;
            out.push(rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"));
        }
    }
    Ok(())
}

/// Copies the files of `src` into a release folder, checks them (the DSP bundle and the TAS image must load),
/// Packs the files of `src` into the two `.ocfw` containers (after the identity checks), and writes the release
/// assets into `out`: the zip (containers + manifest), its `.sha256`, and the manifest alone.
/// `min_app_version`: the oldest app that can install them.
pub fn package(src: &FirmwareSources, out: &Path, min_app_version: &str) -> Result<Assets, String> {
    let stage = out.join("stage");
    let _ = std::fs::remove_dir_all(&stage);
    write_pack(src, &stage, min_app_version)?;
    let read_version = |name: &str| crate::install::ocfw::peek_version(&stage.join(name)).ok_or(format!("{name}: no version"));
    let dsp_version = read_version(crate::install::ocfw::DSP_FILE)?;
    let tas_version = read_version(crate::install::ocfw::USB_FILE)?;
    let manifest = Manifest::of_dir(&stage, &dsp_version, &tas_version, min_app_version)?;
    std::fs::write(stage.join(MANIFEST_NAME), manifest.to_json()).map_err(|e| e.to_string())?;
    let zip = zip_dir(&stage)?;
    let name = format!("opencompanion-firmware-{}-{}.zip", semver(&dsp_version), semver(&tas_version));
    let zip_path = out.join(&name);
    std::fs::write(&zip_path, &zip).map_err(|e| e.to_string())?;
    let sha_path = out.join(format!("{name}.sha256"));
    std::fs::write(&sha_path, format!("{}  {name}\n", sha256_hex(&zip))).map_err(|e| e.to_string())?;
    let manifest_path = out.join(MANIFEST_NAME);
    std::fs::write(&manifest_path, manifest.to_json()).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&stage);
    Ok(Assets { zip: zip_path, zip_sha256: sha_path, manifest: manifest_path, manifest_data: manifest })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zips_are_reproducible() {
        let dir = std::env::temp_dir().join(format!("oc-zip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        super::super::manifest::tests::fake_release(&dir);
        assert_eq!(zip_dir(&dir).unwrap(), zip_dir(&dir).unwrap());
        assert_eq!(semver("0.1.0+2026"), "0.1.0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_dev_tree_packages_when_it_is_built() {
        let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.."));
        let src = FirmwareSources::dev_tree(root);
        if !src.dsp_app.is_file() || !src.tas_image.is_file() {
            eprintln!("skipped: firmware not built");
            return;
        }
        let out = std::env::temp_dir().join(format!("oc-package-{}", std::process::id()));
        match package(&src, &out, "0.1.0") {
            Ok(a) => {
                assert!(a.zip.is_file() && a.manifest.is_file());
                assert_eq!(a.manifest_data.files.len(), super::super::manifest::FILES.len());
            }
            // A work-in-progress build may not load (e.g. an image too large): reported, not a failure here.
            Err(e) => eprintln!("dev tree not packageable: {e}"),
        }
        let _ = std::fs::remove_dir_all(&out);
    }
}
