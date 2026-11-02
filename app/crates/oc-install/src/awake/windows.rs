const ES_CONTINUOUS: u32 = 0x8000_0000;
const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;

#[link(name = "kernel32")]
extern "system" {
    fn SetThreadExecutionState(flags: u32) -> u32;
}

pub struct Guard;

impl Guard {
    pub fn acquire(_reason: &str) -> Result<Self, String> {
        // SAFETY: plain Win32 call with constant flags; it only changes this thread's execution state.
        let prev = unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) };
        if prev == 0 {
            return Err("SetThreadExecutionState failed".into());
        }
        Ok(Self)
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: as above; restores the default state of this thread.
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}
