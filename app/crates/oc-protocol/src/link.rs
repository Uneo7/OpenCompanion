use std::time::Duration;

use crate::Result;

/// Largest payload of a stock Bose report (`64 - report ID - cmd - len`).
pub const BOSE_MAX_PAYLOAD: usize = 61;

pub trait Link {
    /// Sends `cmd` with `data` and returns `(reply command, reply data)`.
    fn transact(&self, cmd: u8, data: &[u8], timeout: Duration) -> Result<(u8, Vec<u8>)>;

    fn max_payload(&self) -> usize {
        BOSE_MAX_PAYLOAD
    }

    fn max_reply(&self) -> usize {
        BOSE_MAX_PAYLOAD
    }

    /// Waits (EEPROM write cycles, settling). Test doubles override this with a no-op.
    fn delay(&self, d: Duration) {
        std::thread::sleep(d);
    }
}

impl<T: Link + ?Sized> Link for &T {
    fn transact(&self, cmd: u8, data: &[u8], timeout: Duration) -> Result<(u8, Vec<u8>)> {
        (**self).transact(cmd, data, timeout)
    }
    fn max_payload(&self) -> usize {
        (**self).max_payload()
    }
    fn max_reply(&self) -> usize {
        (**self).max_reply()
    }
    fn delay(&self, d: Duration) {
        (**self).delay(d)
    }
}
