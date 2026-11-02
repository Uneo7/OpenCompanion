use std::cell::Cell;

use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

use crate::{Error, Result};

struct Guard {
    /// Whether this thread's CoInitializeEx succeeded and must be balanced.
    owned: Cell<Option<bool>>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if self.owned.get() == Some(true) {
            unsafe { CoUninitialize() };
        }
    }
}

thread_local! {
    static GUARD: Guard = const { Guard { owned: Cell::new(None) } };
}

/// Initializes COM (MTA) on the calling thread once. A thread already in an
/// STA (e.g. the GUI thread after OleInitialize) is fine as is: Core Audio
/// objects are usable from both apartment types.
pub fn ensure() -> Result<()> {
    GUARD.with(|g| {
        if g.owned.get().is_some() {
            return Ok(());
        }
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_ok() {
            g.owned.set(Some(true));
            Ok(())
        } else if hr == RPC_E_CHANGED_MODE {
            g.owned.set(Some(false));
            Ok(())
        } else {
            Err(Error::os("CoInitializeEx", windows_core::Error::from(hr)))
        }
    })
}
