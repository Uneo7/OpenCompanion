#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_void};

pub type OSStatus = i32;
pub type AudioObjectID = u32;
pub type AudioObjectPropertySelector = u32;
pub type AudioObjectPropertyScope = u32;
pub type AudioObjectPropertyElement = u32;
pub type Boolean = u8;
pub type CFStringRef = *const c_void;
pub type CFIndex = isize;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioObjectPropertyAddress {
    pub mSelector: AudioObjectPropertySelector,
    pub mScope: AudioObjectPropertyScope,
    pub mElement: AudioObjectPropertyElement,
}

pub type AudioObjectPropertyListenerProc = unsafe extern "C" fn(
    in_object_id: AudioObjectID,
    in_number_addresses: u32,
    in_addresses: *const AudioObjectPropertyAddress,
    in_client_data: *mut c_void,
) -> OSStatus;

const fn fourcc(s: &[u8; 4]) -> u32 {
    ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | (s[3] as u32)
}

pub const kAudioObjectSystemObject: AudioObjectID = 1;
pub const kAudioObjectPropertyElementMain: AudioObjectPropertyElement = 0;

pub const kAudioObjectPropertyScopeGlobal: AudioObjectPropertyScope = fourcc(b"glob");
pub const kAudioDevicePropertyScopeOutput: AudioObjectPropertyScope = fourcc(b"outp");

pub const kAudioHardwarePropertyDevices: AudioObjectPropertySelector = fourcc(b"dev#");
pub const kAudioObjectPropertyName: AudioObjectPropertySelector = fourcc(b"lnam");
pub const kAudioDevicePropertyDeviceUID: AudioObjectPropertySelector = fourcc(b"uid ");
pub const kAudioDevicePropertyModelUID: AudioObjectPropertySelector = fourcc(b"muid");
pub const kAudioDevicePropertyTransportType: AudioObjectPropertySelector = fourcc(b"tran");
pub const kAudioDevicePropertyStreams: AudioObjectPropertySelector = fourcc(b"stm#");
pub const kAudioDevicePropertyPreferredChannelsForStereo: AudioObjectPropertySelector =
    fourcc(b"dch2");
pub const kAudioDevicePropertyVolumeScalar: AudioObjectPropertySelector = fourcc(b"volm");
pub const kAudioDevicePropertyVolumeDecibels: AudioObjectPropertySelector = fourcc(b"vdb ");
pub const kAudioDevicePropertyMute: AudioObjectPropertySelector = fourcc(b"mute");

pub const kAudioDeviceTransportTypeUSB: u32 = fourcc(b"usb ");

pub const kCFStringEncodingUTF8: u32 = 0x0800_0100;

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    pub fn AudioObjectHasProperty(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
    ) -> Boolean;
    pub fn AudioObjectIsPropertySettable(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
        outIsSettable: *mut Boolean,
    ) -> OSStatus;
    pub fn AudioObjectGetPropertyDataSize(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
        inQualifierDataSize: u32,
        inQualifierData: *const c_void,
        outDataSize: *mut u32,
    ) -> OSStatus;
    pub fn AudioObjectGetPropertyData(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
        inQualifierDataSize: u32,
        inQualifierData: *const c_void,
        ioDataSize: *mut u32,
        outData: *mut c_void,
    ) -> OSStatus;
    pub fn AudioObjectSetPropertyData(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
        inQualifierDataSize: u32,
        inQualifierData: *const c_void,
        inDataSize: u32,
        inData: *const c_void,
    ) -> OSStatus;
    pub fn AudioObjectAddPropertyListener(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
        inListener: AudioObjectPropertyListenerProc,
        inClientData: *mut c_void,
    ) -> OSStatus;
    pub fn AudioObjectRemovePropertyListener(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
        inListener: AudioObjectPropertyListenerProc,
        inClientData: *mut c_void,
    ) -> OSStatus;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub fn CFStringGetLength(theString: CFStringRef) -> CFIndex;
    pub fn CFStringGetMaximumSizeForEncoding(length: CFIndex, encoding: u32) -> CFIndex;
    pub fn CFStringGetCString(
        theString: CFStringRef,
        buffer: *mut c_char,
        bufferSize: CFIndex,
        encoding: u32,
    ) -> Boolean;
    pub fn CFRelease(cf: *const c_void);
}
