use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use oc_install::backup::{default_root, load, stock_backup_status, BackupFiles};

pub fn stock_backup(from: Option<&Path>, root: Option<&Path>) -> anyhow::Result<BackupFiles> {
    let dir: PathBuf = match from {
        Some(d) => d.to_path_buf(),
        None => {
            let root = root.map(Path::to_path_buf).unwrap_or_else(default_root);
            match stock_backup_status(&root) {
                Some(e) => e.dir,
                None => bail!(
                    "no verified backup of the Bose firmware in {}: run `oc backup` while the speaker still runs \
                     the Bose firmware",
                    root.display()
                ),
            }
        }
    };
    let files = load(&dir).with_context(|| format!("backup {}", dir.display()))?;
    println!("backup: {}", dir.display());
    Ok(files)
}
