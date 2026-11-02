use windows::Win32::Devices::FunctionDiscovery::{
    PKEY_DeviceInterface_FriendlyName, PKEY_Device_DeviceDesc, PKEY_Device_FriendlyName,
    PKEY_Device_InstanceId,
};
use windows::Win32::Media::Audio::{
    eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, PROPERTYKEY};
use windows_core::{BSTR, HSTRING, PROPVARIANT};

use super::com;
use crate::matcher::{parse_windows_hwid, DeviceInfo};
use crate::{Error, Result};

fn enumerator() -> Result<IMMDeviceEnumerator> {
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
        .map_err(|e| Error::os("CoCreateInstance(MMDeviceEnumerator)", e))
}

pub fn list_devices() -> Result<Vec<DeviceInfo>> {
    com::ensure()?;
    let collection = unsafe { enumerator()?.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) }
        .map_err(|e| Error::os("EnumAudioEndpoints", e))?;
    let count = unsafe { collection.GetCount() }.map_err(|e| Error::os("GetCount", e))?;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        let device = unsafe { collection.Item(i) }.map_err(|e| Error::os("Item", e))?;
        // A single unreadable endpoint must not hide the others.
        if let Ok(info) = describe(&device) {
            out.push(info);
        }
    }
    Ok(out)
}

pub fn device_by_id(id: &str) -> Result<IMMDevice> {
    unsafe { enumerator()?.GetDevice(&HSTRING::from(id)) }
        .map_err(|e| Error::os("IMMDeviceEnumerator::GetDevice", e))
}

fn describe(device: &IMMDevice) -> Result<DeviceInfo> {
    let id = unsafe {
        let pwstr = device
            .GetId()
            .map_err(|e| Error::os("IMMDevice::GetId", e))?;
        let id = pwstr.to_string().unwrap_or_default();
        CoTaskMemFree(Some(pwstr.0 as *const _));
        id
    };
    let store = unsafe { device.OpenPropertyStore(STGM_READ) }
        .map_err(|e| Error::os("OpenPropertyStore", e))?;

    // DeviceInterface_FriendlyName is the USB product string ("Companion 5
    // dev"); Device_FriendlyName is "Speakers (Companion 5 dev)" and may be
    // renamed by the user; DeviceDesc is the endpoint label ("Speakers").
    let names = [
        &PKEY_DeviceInterface_FriendlyName,
        &PKEY_Device_FriendlyName,
        &PKEY_Device_DeviceDesc,
    ]
    .into_iter()
    .filter_map(|k| string_prop(&store, k))
    .filter(|s| !s.is_empty())
    .collect();

    // The endpoint's own InstanceId is SWD\MMDEVAPI\..., not USB. The USB
    // devnode ID (e.g. "{1}.USB\VID_1209&PID_0001&MI_00\...", PID_0002 in audio mode 2.1) sits in other
    // endpoint properties, so scan every string value for a USB hardware ID.
    let usb = string_prop(&store, &PKEY_Device_InstanceId)
        .and_then(|s| parse_windows_hwid(&s))
        .or_else(|| all_string_props(&store).find_map(|s| parse_windows_hwid(&s)));

    Ok(DeviceInfo { id, names, usb })
}

fn to_string(value: &PROPVARIANT) -> Option<String> {
    // SAFETY: `vt` is the discriminant, valid for any PROPVARIANT.
    if unsafe { value.as_raw().Anonymous.Anonymous.vt } != VT_LPWSTR.0 {
        return None;
    }
    BSTR::try_from(value).ok().map(|b| b.to_string())
}

fn string_prop(store: &IPropertyStore, key: &PROPERTYKEY) -> Option<String> {
    let value = unsafe { store.GetValue(key) }.ok()?;
    to_string(&value)
}

fn all_string_props(store: &IPropertyStore) -> impl Iterator<Item = String> + '_ {
    let count = unsafe { store.GetCount() }.unwrap_or(0);
    (0..count).filter_map(move |i| {
        let mut key = PROPERTYKEY::default();
        unsafe { store.GetAt(i, &mut key) }.ok()?;
        string_prop(store, &key)
    })
}
