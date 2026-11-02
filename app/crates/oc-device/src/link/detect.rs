use oc_install::status::TAS_DFU_IDS;
use oc_protocol::oc_hid::OcDevice;
use oc_protocol::transport::Companion;

use super::Conn;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presence {
    Absent,
    /// OpenCompanion TAS firmware: 05A7:1020 with our bcdDevice / product string (0.5.0+), or 1209:0001 / 0002
    /// (0.4.x), `oc_hid::is_opencompanion`.
    OpenCompanion,
    /// Stock Bose TAS firmware (05A7:1020): the app offers the install.
    Stock,
    RomDfu { vid: u16, pid: u16 },
    Inaccessible(String),
}

impl Presence {
    pub fn is_connected(&self) -> bool {
        matches!(self, Presence::OpenCompanion | Presence::Stock)
    }
}

pub fn probe() -> (Presence, Option<Conn>) {
    match OcDevice::open() {
        Ok(d) => return (Presence::OpenCompanion, Some(Conn::Oc(d))),
        Err(oc_protocol::Error::NotFound) => {}
        Err(e) => return (Presence::Inaccessible(e.to_string()), None),
    }
    match Companion::open() {
        Ok(c) => return (Presence::Stock, Some(Conn::Stock(c))),
        Err(oc_protocol::Error::NotFound) => {}
        Err(e) => return (Presence::Inaccessible(e.to_string()), None),
    }
    // No vendor HID: the ROM DFU has none, look at the raw USB identities.
    let ids = oc_dfu::present_ids().unwrap_or_default();
    match ids.iter().find(|id| TAS_DFU_IDS.contains(id)) {
        Some(&(vid, pid)) => (Presence::RomDfu { vid, pid }, None),
        None => (Presence::Absent, None),
    }
}
