//! No Bose firmware is shipped with OpenCompanion: a restore can only use backups read from this speaker.
//! U7001 is read through the RAM-only updater, never via `UPD`: a backup writes no EEPROM byte.
//! A backup folder is never overwritten; `checksums.txt` is written last, a folder without it is ignored.

mod auto;
pub mod files;
mod read;
mod stamp;

pub mod raw;
pub use auto::{ensure_stock_backup, stock_backup_status, StockBackup};
pub use files::{default_root, list, load, save, BackupEntry, BackupFiles, Content, DSP_FILE, SUMS_FILE, TAS_FILE, VERSION_FILE};
pub use read::{read_dsp, read_tas, speaker_id, DspDump};
pub use stamp::{stamp_at, stamp_now};
