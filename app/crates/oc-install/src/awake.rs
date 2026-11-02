//! Keeps the host awake during EEPROM writes: a suspended host can make the stock TAS firmware run its
//! standby logic, which resets the DSP (fatal inside the migration's critical window).

#[cfg(target_os = "linux")]
#[path = "awake/linux.rs"]
mod imp;
#[cfg(target_os = "macos")]
#[path = "awake/macos.rs"]
mod imp;
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
#[path = "awake/other.rs"]
mod imp;
#[cfg(target_os = "windows")]
#[path = "awake/windows.rs"]
mod imp;

pub struct KeepAwake {
    _guard: imp::Guard,
}

impl KeepAwake {
    pub fn acquire(reason: &str) -> Result<Self, String> {
        Ok(Self { _guard: imp::Guard::acquire(reason)? })
    }
}
