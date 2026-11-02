//! Single source of every OpenCompanion path (`oc_device::paths` re-exports it): nothing else builds them.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Folder name under the OS folders: lower case on Linux (XDG custom), capitalised elsewhere.
pub fn app_dir_name() -> &'static str {
    if cfg!(target_os = "linux") {
        "opencompanion"
    } else {
        "OpenCompanion"
    }
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(std::env::temp_dir)
}

pub fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join(app_dir_name())
}

pub fn backups_dir() -> PathBuf {
    data_dir().join("backups")
}

pub fn firmware_dir() -> PathBuf {
    data_dir().join("firmware")
}

pub fn profiles_dir() -> PathBuf {
    data_dir().join("profiles")
}

pub fn bases_dir() -> PathBuf {
    data_dir().join("bases")
}

pub fn config_dir() -> PathBuf {
    if cfg!(target_os = "linux") {
        dirs::config_dir().unwrap_or_else(|| home().join(".config")).join(app_dir_name())
    } else {
        data_dir()
    }
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.json")
}

pub fn log_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("OC_LOG_DIR") {
        return PathBuf::from(d);
    }
    if cfg!(target_os = "windows") {
        dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("OpenCompanion").join("logs")
    } else if cfg!(target_os = "macos") {
        home().join("Library").join("Logs").join("OpenCompanion")
    } else {
        dirs::state_dir().unwrap_or_else(|| home().join(".local").join("state")).join(app_dir_name())
    }
}

/// Moves `from` into `to`, entry by entry, never overwriting: a file already present in `to` keeps its name and
/// the moved one gets a `.migrated`, `.migrated-2`… suffix. Empty source folders are removed. Returns what moved.
fn merge_into(from: &Path, to: &Path, moved: &mut Vec<String>) -> io::Result<()> {
    if !from.is_dir() || same(from, to) {
        return Ok(());
    }
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        let src = e.path();
        let mut dst = to.join(e.file_name());
        if src.is_dir() {
            merge_into(&src, &dst, moved)?;
            continue;
        }
        let mut k = 1;
        while dst.exists() {
            let name = e.file_name().to_string_lossy().into_owned();
            dst = to.join(if k == 1 { format!("{name}.migrated") } else { format!("{name}.migrated-{k}") });
            k += 1;
        }
        move_file(&src, &dst)?;
        moved.push(format!("{} -> {}", src.display(), dst.display()));
    }
    let _ = fs::remove_dir(from); // only when empty
    Ok(())
}

fn move_file(src: &Path, dst: &Path) -> io::Result<()> {
    if fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    // Another volume: copy, then remove.
    fs::copy(src, dst)?;
    fs::remove_file(src)
}

fn same(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

struct Old {
    data: PathBuf,
    config: PathBuf,
}

fn old() -> Option<Old> {
    Some(Old {
        data: dirs::data_dir()?.join("OpenCompanion"),
        config: dirs::config_dir()?.join("OpenCompanion").join("config.json"),
    })
}

/// Idempotent, never overwrites; returns what moved (for the log).
pub fn migrate() -> io::Result<Vec<String>> {
    let Some(old) = old() else { return Ok(Vec::new()) };
    migrate_from(&old, &data_dir(), &config_file(), &log_dir())
}

fn migrate_from(old: &Old, data: &Path, config: &Path, logs: &Path) -> io::Result<Vec<String>> {
    let mut moved = Vec::new();
    // Logs first (they sat inside the old data folder).
    merge_into(&old.data.join("logs"), logs, &mut moved)?;
    // Data folder renamed (Linux: case change; elsewhere the same folder, nothing to do).
    merge_into(&old.data, data, &mut moved)?;
    if old.config.is_file() && !same(&old.config, config) {
        if let Some(dir) = config.parent() {
            fs::create_dir_all(dir)?;
        }
        if config.exists() {
            let keep = config.with_extension("json.migrated");
            move_file(&old.config, &keep)?;
            moved.push(format!("{} -> {}", old.config.display(), keep.display()));
        } else {
            move_file(&old.config, config)?;
            moved.push(format!("{} -> {}", old.config.display(), config.display()));
        }
        if let Some(d) = old.config.parent() {
            let _ = fs::remove_dir(d);
        }
    }
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("oc-paths-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn layout() {
        assert!(backups_dir().starts_with(data_dir()) && profiles_dir().starts_with(data_dir()));
        assert!(bases_dir().ends_with("bases"));
        assert!(config_file().ends_with("config.json"));
        if cfg!(target_os = "linux") {
            assert!(data_dir().ends_with("opencompanion"));
        } else {
            assert!(data_dir().ends_with("OpenCompanion"));
            assert_eq!(config_dir(), data_dir());
        }
    }

    #[test]
    fn migration_moves_everything_once_and_never_overwrites() {
        let root = tmp("migrate");
        let old = Old { data: root.join("OpenCompanion"), config: root.join("cfg/OpenCompanion/config.json") };
        // Distinct names (a case change alone is the same folder on Windows and macOS).
        let (data, config, logs) = (root.join("data-new"), root.join("cfg/new/config.json"), root.join("state/oc"));
        fs::create_dir_all(old.data.join("logs")).unwrap();
        fs::create_dir_all(old.data.join("backups/spk/2026-11-01_120000")).unwrap();
        fs::write(old.data.join("logs/install.log"), "old log").unwrap();
        fs::write(old.data.join("backups/spk/2026-11-01_120000/u6002_tas.bin"), "tas").unwrap();
        fs::create_dir_all(old.config.parent().unwrap()).unwrap();
        fs::write(&old.config, "{}").unwrap();
        fs::create_dir_all(&logs).unwrap();
        fs::write(logs.join("install.log"), "new log").unwrap();

        let moved = migrate_from(&old, &data, &config, &logs).unwrap();
        assert_eq!(moved.len(), 3, "{moved:?}");
        assert_eq!(fs::read_to_string(logs.join("install.log")).unwrap(), "new log");
        assert_eq!(fs::read_to_string(logs.join("install.log.migrated")).unwrap(), "old log");
        assert_eq!(fs::read_to_string(data.join("backups/spk/2026-11-01_120000/u6002_tas.bin")).unwrap(), "tas");
        assert_eq!(fs::read_to_string(&config).unwrap(), "{}");
        assert!(!old.data.exists() && !old.config.exists());
        assert!(migrate_from(&old, &data, &config, &logs).unwrap().is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn same_folder_is_left_alone() {
        let root = tmp("same");
        let d = root.join("OpenCompanion");
        fs::create_dir_all(d.join("backups")).unwrap();
        fs::write(d.join("backups/x"), "1").unwrap();
        let old = Old { data: d.clone(), config: root.join("none.json") };
        let moved = migrate_from(&old, &d, &root.join("cfg.json"), &root.join("logs")).unwrap();
        assert!(moved.is_empty());
        assert!(d.join("backups/x").exists());
        let _ = fs::remove_dir_all(&root);
    }
}
