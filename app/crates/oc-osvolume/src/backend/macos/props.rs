use std::ffi::{c_char, c_void, CStr};
use std::mem::{size_of, MaybeUninit};

use super::ffi::*;
use crate::{Error, Result};

pub fn addr(
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
    element: AudioObjectPropertyElement,
) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: scope,
        mElement: element,
    }
}

fn check(api: &'static str, status: OSStatus) -> Result<()> {
    if status == 0 {
        Ok(())
    } else {
        Err(Error::os(api, format!("OSStatus {status}")))
    }
}

pub fn has(obj: AudioObjectID, a: &AudioObjectPropertyAddress) -> bool {
    unsafe { AudioObjectHasProperty(obj, a) != 0 }
}

pub fn settable(obj: AudioObjectID, a: &AudioObjectPropertyAddress) -> bool {
    let mut out: Boolean = 0;
    let status = unsafe { AudioObjectIsPropertySettable(obj, a, &mut out) };
    status == 0 && out != 0
}

/// Reads a plain-old-data property of exactly `size_of::<T>()` bytes.
pub fn get<T: Copy>(obj: AudioObjectID, a: &AudioObjectPropertyAddress) -> Result<T> {
    let mut value = MaybeUninit::<T>::uninit();
    let mut size = size_of::<T>() as u32;
    check("AudioObjectGetPropertyData", unsafe {
        AudioObjectGetPropertyData(
            obj,
            a,
            0,
            std::ptr::null(),
            &mut size,
            value.as_mut_ptr().cast(),
        )
    })?;
    if size as usize != size_of::<T>() {
        return Err(Error::os("AudioObjectGetPropertyData", "unexpected size"));
    }
    // SAFETY: the HAL filled exactly size_of::<T>() bytes.
    Ok(unsafe { value.assume_init() })
}

pub fn set<T: Copy>(obj: AudioObjectID, a: &AudioObjectPropertyAddress, value: T) -> Result<()> {
    check("AudioObjectSetPropertyData", unsafe {
        AudioObjectSetPropertyData(
            obj,
            a,
            0,
            std::ptr::null(),
            size_of::<T>() as u32,
            (&value as *const T).cast(),
        )
    })
}

pub fn get_vec<T: Copy + Default>(
    obj: AudioObjectID,
    a: &AudioObjectPropertyAddress,
) -> Result<Vec<T>> {
    let mut size = 0u32;
    check("AudioObjectGetPropertyDataSize", unsafe {
        AudioObjectGetPropertyDataSize(obj, a, 0, std::ptr::null(), &mut size)
    })?;
    let mut out = vec![T::default(); size as usize / size_of::<T>()];
    if out.is_empty() {
        return Ok(out);
    }
    check("AudioObjectGetPropertyData", unsafe {
        AudioObjectGetPropertyData(
            obj,
            a,
            0,
            std::ptr::null(),
            &mut size,
            out.as_mut_ptr().cast(),
        )
    })?;
    out.truncate(size as usize / size_of::<T>());
    Ok(out)
}

/// The CFString is returned retained: released here.
pub fn get_string(obj: AudioObjectID, a: &AudioObjectPropertyAddress) -> Result<String> {
    let cf: CFStringRef = get(obj, a)?;
    if cf.is_null() {
        return Ok(String::new());
    }
    // SAFETY: `cf` is a valid, owned CFString.
    let s = unsafe { cfstring_to_string(cf) };
    unsafe { CFRelease(cf) };
    Ok(s)
}

unsafe fn cfstring_to_string(cf: CFStringRef) -> String {
    let len = CFStringGetLength(cf);
    let cap = CFStringGetMaximumSizeForEncoding(len, kCFStringEncodingUTF8) + 1;
    let mut buf = vec![0 as c_char; cap.max(1) as usize];
    if CFStringGetCString(cf, buf.as_mut_ptr(), cap, kCFStringEncodingUTF8) == 0 {
        return String::new();
    }
    CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
}

pub fn add_listener(
    obj: AudioObjectID,
    a: &AudioObjectPropertyAddress,
    listener: AudioObjectPropertyListenerProc,
    data: *mut c_void,
) -> Result<()> {
    check("AudioObjectAddPropertyListener", unsafe {
        AudioObjectAddPropertyListener(obj, a, listener, data)
    })
}

pub fn remove_listener(
    obj: AudioObjectID,
    a: &AudioObjectPropertyAddress,
    listener: AudioObjectPropertyListenerProc,
    data: *mut c_void,
) {
    unsafe { AudioObjectRemovePropertyListener(obj, a, listener, data) };
}
