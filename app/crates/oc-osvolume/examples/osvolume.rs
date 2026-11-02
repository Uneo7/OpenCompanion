//! `cargo run -p oc-osvolume --example osvolume [-- watch]`: lists output devices, reads (never writes) the volume.

use oc_osvolume::{find_device, list_devices, DeviceMatch};

fn main() -> Result<(), oc_osvolume::Error> {
    for d in list_devices()? {
        let usb = d.usb.map(|u| u.to_string()).unwrap_or_else(|| "-".into());
        println!("{usb:9} {:?}  [{}]", d.names, d.id);
    }
    let Some(dev) = find_device(&DeviceMatch::companion())? else {
        println!("speaker not found");
        return Ok(());
    };
    println!("speaker: {} -> {:?}", dev.info().display_name(), dev.get()?);
    if std::env::args().any(|a| a == "watch") {
        let _sub = dev.subscribe(|s| println!("changed: {s:?}"))?;
        println!("watching, press Enter to quit");
        let _ = std::io::stdin().read_line(&mut String::new());
    }
    Ok(())
}
