use alsa::card::{Card, Iter};

use crate::matcher::{parse_colon_pair, DeviceInfo};
use crate::{Error, Result};

pub fn ctl_name(card: &Card) -> String {
    format!("hw:{}", card.get_index())
}

/// `snd-usb-audio` publishes "VVVV:PPPP" in `/proc/asound/cardN/usbid`.
fn usb_id(index: i32) -> Option<crate::UsbId> {
    let text = std::fs::read_to_string(format!("/proc/asound/card{index}/usbid")).ok()?;
    parse_colon_pair(&text)
}

pub fn list_devices() -> Result<Vec<DeviceInfo>> {
    let mut out = Vec::new();
    for card in Iter::new() {
        let card = card.map_err(|e| Error::os("snd_card_next", e))?;
        // Short name is the USB product string ("Companion 5 dev"), long name
        // adds the vendor and bus location.
        let names = [card.get_name(), card.get_longname()]
            .into_iter()
            .filter_map(|n| n.ok())
            .filter(|n| !n.is_empty())
            .collect();
        out.push(DeviceInfo {
            id: ctl_name(&card),
            names,
            usb: usb_id(card.get_index()),
        });
    }
    Ok(out)
}
