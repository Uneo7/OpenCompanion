use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UsbId {
    pub vid: u16,
    pub pid: u16,
}

impl UsbId {
    pub const fn new(vid: u16, pid: u16) -> Self {
        Self { vid, pid }
    }
}

impl fmt::Display for UsbId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04x}:{:04x}", self.vid, self.pid)
    }
}

/// A USB ID match wins over a name match.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceMatch {
    /// Case-insensitive substring of the device name (OS names often wrap the
    /// USB product string, e.g. "Speakers (Companion 5 (OpenCompanion))").
    pub name: Option<String>,
    /// Any of these USB IDs (empty: no USB match).
    pub usb: Vec<UsbId>,
}

impl DeviceMatch {
    /// TAS firmware 0.4.x (pid.codes test VID/PIDs), audio modes 5.1 / 2.1.
    pub const DEV_USB_ID: UsbId = UsbId::new(0x1209, 0x0001);
    pub const DEV_USB_ID_21: UsbId = UsbId::new(0x1209, 0x0002);
    /// Product-string substring of TAS firmware 0.5.0+ ("Companion 5 (OpenCompanion)").
    pub const DEV_NAME: &'static str = "OpenCompanion";
    /// 05A7:1020: the stock Bose firmware ("Bose USB Audio", the speaker right after a restore) and, since TAS
    /// firmware 0.5.0, OpenCompanion too (both audio modes). Telling them apart needs the HID enumeration data
    /// (bcdDevice, product string: `oc_protocol::oc_hid::is_opencompanion`), not this matcher.
    pub const BOSE_USB_ID: UsbId = UsbId::new(0x05a7, 0x1020);

    pub fn companion() -> Self {
        Self {
            name: Some(Self::DEV_NAME.to_owned()),
            usb: vec![Self::DEV_USB_ID, Self::DEV_USB_ID_21, Self::BOSE_USB_ID],
        }
    }

    pub fn by_name(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            usb: Vec::new(),
        }
    }

    pub fn by_usb(vid: u16, pid: u16) -> Self {
        Self {
            name: None,
            usb: vec![UsbId::new(vid, pid)],
        }
    }

    /// 2 = USB ID match, 1 = name match, 0 = no match.
    pub fn score(&self, dev: &DeviceInfo) -> u8 {
        if let Some(have) = dev.usb {
            if self.usb.contains(&have) {
                return 2;
            }
        }
        if let Some(needle) = self.name.as_deref().map(str::trim) {
            if !needle.is_empty() {
                let needle = needle.to_lowercase();
                if dev.names.iter().any(|n| n.to_lowercase().contains(&needle)) {
                    return 1;
                }
            }
        }
        0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Backend-specific stable ID (Windows endpoint ID, macOS device UID,
    /// Linux ALSA card index).
    pub id: String,
    /// Most specific first.
    pub names: Vec<String>,
    pub usb: Option<UsbId>,
}

impl DeviceInfo {
    pub fn display_name(&self) -> &str {
        self.names.first().map(String::as_str).unwrap_or(&self.id)
    }
}

/// Highest score, first on ties.
pub fn best_match(devices: &[DeviceInfo], m: &DeviceMatch) -> Option<usize> {
    devices
        .iter()
        .enumerate()
        .map(|(i, d)| (m.score(d), i))
        .filter(|&(s, _)| s > 0)
        .fold(None, |best: Option<(u8, usize)>, cur| match best {
            Some(b) if b.0 >= cur.0 => Some(b),
            _ => Some(cur),
        })
        .map(|(_, i)| i)
}

fn hex4_after(upper: &str, tag: &str) -> Option<u16> {
    let start = upper.find(tag)? + tag.len();
    let digits = upper.get(start..start + 4)?;
    u16::from_str_radix(digits, 16).ok()
}

/// Parses a Windows PnP hardware/instance ID such as
/// `USB\VID_1209&PID_0001&MI_00\7&1a2b3c&0&0000` or
/// `{1}.USB\VID_1209&PID_0001&MI_00\...` (endpoint property values).
pub fn parse_windows_hwid(s: &str) -> Option<UsbId> {
    let upper = s.to_ascii_uppercase();
    if !upper.contains("USB") {
        return None;
    }
    Some(UsbId::new(
        hex4_after(&upper, "VID_")?,
        hex4_after(&upper, "PID_")?,
    ))
}

/// Parses a `VVVV:PPPP` pair: Linux `/proc/asound/cardN/usbid`, or the tail of
/// a macOS model UID such as `Companion 5 dev:1209:1` (hex, unpadded there).
pub fn parse_colon_pair(s: &str) -> Option<UsbId> {
    let mut parts = s.trim().rsplitn(3, ':');
    let pid = parts.next()?;
    let vid = parts.next()?;
    let hex = |t: &str| {
        let t = t.trim();
        if (1..=4).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_hexdigit()) {
            u16::from_str_radix(t, 16).ok()
        } else {
            None
        }
    };
    Some(UsbId::new(hex(vid)?, hex(pid)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(id: &str, names: &[&str], usb: Option<UsbId>) -> DeviceInfo {
        DeviceInfo {
            id: id.into(),
            names: names.iter().map(|s| s.to_string()).collect(),
            usb,
        }
    }

    #[test]
    fn windows_hwid() {
        let id = UsbId::new(0x1209, 0x0001);
        assert_eq!(
            parse_windows_hwid(r"USB\VID_1209&PID_0001&MI_00\7&1a&0&0000"),
            Some(id)
        );
        assert_eq!(
            parse_windows_hwid(r"{1}.USB\VID_1209&PID_0001&MI_00\7&1a"),
            Some(id)
        );
        assert_eq!(parse_windows_hwid(r"usb\vid_1209&pid_0001"), Some(id));
        assert_eq!(
            parse_windows_hwid(r"USB\VID_05A7&PID_40FE"),
            Some(UsbId::new(0x05a7, 0x40fe))
        );
        assert_eq!(
            parse_windows_hwid(r"SWD\MMDEVAPI\{0.0.0.00000000}.{abcd}"),
            None
        );
        assert_eq!(
            parse_windows_hwid(r"HDAUDIO\FUNC_01&VEN_10EC&DEV_0897"),
            None
        );
        assert_eq!(parse_windows_hwid(r"USB\VID_12"), None);
    }

    #[test]
    fn colon_pair() {
        let id = UsbId::new(0x1209, 0x0001);
        assert_eq!(parse_colon_pair("1209:0001\n"), Some(id));
        assert_eq!(parse_colon_pair("Companion 5 dev:1209:1"), Some(id));
        assert_eq!(parse_colon_pair("AppleUSBAudioEngine:x:y:z"), None);
        assert_eq!(parse_colon_pair("BuiltInSpeakerDevice"), None);
        assert_eq!(parse_colon_pair("12345:1"), None);
    }

    #[test]
    fn usb_beats_name() {
        let m = DeviceMatch::companion();
        let devs = [
            dev("a", &["Speakers (Realtek)"], None),
            dev("b", &["Speakers (Companion 5 (OpenCompanion))"], None),
            dev("c", &["Speakers (Renamed)"], Some(DeviceMatch::DEV_USB_ID)),
        ];
        assert_eq!(best_match(&devs, &m), Some(2));
        let mode21 = [dev("d", &["Speakers (Renamed)"], Some(DeviceMatch::DEV_USB_ID_21))];
        assert_eq!(best_match(&mode21, &m), Some(0), "audio mode 2.1 has its own PID");
        assert_eq!(best_match(&devs[..2], &m), Some(1));
        assert_eq!(best_match(&devs[..1], &m), None);
    }

    #[test]
    fn name_is_case_insensitive_and_wrong_usb_falls_back() {
        let m = DeviceMatch::companion();
        let d = dev(
            "x",
            &["SPEAKERS (companion 5 (OPENCOMPANION))"],
            Some(UsbId::new(0x1209, 0x0003)),
        );
        assert_eq!(m.score(&d), 1);
        assert_eq!(DeviceMatch::by_usb(0x1209, 0x0003).score(&d), 2);
        assert_eq!(DeviceMatch::by_name("  ").score(&d), 0);
        assert_eq!(DeviceMatch::default().score(&d), 0);
    }

    #[test]
    fn first_wins_on_ties() {
        let m = DeviceMatch::by_name("companion");
        let devs = [
            dev("a", &["Companion A"], None),
            dev("b", &["Companion B"], None),
        ];
        assert_eq!(best_match(&devs, &m), Some(0));
    }

    #[test]
    fn display() {
        assert_eq!(DeviceMatch::DEV_USB_ID.to_string(), "1209:0001");
        assert_eq!(dev("id", &[], None).display_name(), "id");
    }
}
