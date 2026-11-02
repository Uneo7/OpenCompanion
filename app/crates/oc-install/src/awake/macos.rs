use std::ffi::{c_char, c_void, CString};

type CFStringRef = *const c_void;
const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_IOPM_ASSERTION_LEVEL_ON: u32 = 255;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithCString(alloc: *const c_void, s: *const c_char, encoding: u32) -> CFStringRef;
    fn CFRelease(cf: *const c_void);
}

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOPMAssertionCreateWithName(kind: CFStringRef, level: u32, name: CFStringRef, id: *mut u32) -> i32;
    fn IOPMAssertionRelease(id: u32) -> i32;
}

fn cfstr(s: &str) -> Result<CFStringRef, String> {
    let c = CString::new(s.replace('\0', " ")).map_err(|e| e.to_string())?;
    // SAFETY: valid NUL-terminated string, default allocator.
    let r = unsafe { CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), K_CF_STRING_ENCODING_UTF8) };
    if r.is_null() {
        Err("CFStringCreateWithCString failed".into())
    } else {
        Ok(r)
    }
}

pub struct Guard {
    id: u32,
}

impl Guard {
    pub fn acquire(reason: &str) -> Result<Self, String> {
        let kind = cfstr("PreventUserIdleSystemSleep")?;
        let name = match cfstr(reason) {
            Ok(n) => n,
            Err(e) => {
                // SAFETY: `kind` was created above and is released once.
                unsafe { CFRelease(kind) };
                return Err(e);
            }
        };
        let mut id = 0u32;
        // SAFETY: both CF strings are valid; `id` is a valid out pointer.
        let rc = unsafe { IOPMAssertionCreateWithName(kind, K_IOPM_ASSERTION_LEVEL_ON, name, &mut id) };
        // SAFETY: the assertion keeps its own references; ours are released once.
        unsafe {
            CFRelease(kind);
            CFRelease(name);
        }
        if rc != 0 {
            return Err(format!("IOPMAssertionCreateWithName failed (0x{rc:08x})"));
        }
        Ok(Self { id })
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: `id` comes from a successful IOPMAssertionCreateWithName.
        unsafe {
            IOPMAssertionRelease(self.id);
        }
    }
}
