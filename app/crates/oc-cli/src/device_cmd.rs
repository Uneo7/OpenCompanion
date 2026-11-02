use std::path::Path;

use oc_protocol::dsp::DspConsole;
use oc_protocol::tas_eeprom;
use oc_protocol::transport::Companion;

pub fn probe() -> anyhow::Result<()> {
    let link = Companion::open()?;
    let dsp = DspConsole::new(&link);
    println!("SVM: {}", dsp.text("SVM")?);
    println!("SYS:\n{}", dsp.text("SYS")?);
    Ok(())
}

pub fn dump_tas(output: &Path) -> anyhow::Result<()> {
    let link = Companion::open()?;
    let data = tas_eeprom::dump(&link, |done, total| {
        if done % 1024 == 0 {
            println!("  {done}/{total}");
        }
    })?;
    std::fs::write(output, &data)?;
    println!("{}: {} bytes", output.display(), data.len());
    Ok(())
}
