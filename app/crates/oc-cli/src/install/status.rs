use oc_install::status::{self, TasStatus};

use super::connect::Conn;

pub fn run() -> anyhow::Result<()> {
    let tas = match Conn::open()? {
        Some(conn) => {
            let tas = match &conn {
                Conn::Bose(c) => status::tas_bose(c),
                Conn::Oc(d) => status::tas_oc(d),
            };
            println!("TAS (U6000 / EEPROM U6002): {tas}");
            println!("DSP (U7001):                {}", conn.with_dsp(status::dsp));
            tas
        }
        None => {
            let ids = oc_dfu::present_ids().unwrap_or_default();
            let tas = status::tas_without_hid(&ids);
            println!("TAS (U6000 / EEPROM U6002): {tas}");
            println!("DSP (U7001):                unreachable without the TAS vendor HID interface");
            tas
        }
    };
    if matches!(tas, TasStatus::RomDfu { .. }) {
        println!("Recovery needs the ROM DFU (developer tools): opencompanion/docs/recovery.md");
    }
    Ok(())
}
