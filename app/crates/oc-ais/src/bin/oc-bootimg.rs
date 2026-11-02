use std::collections::HashMap;
use std::process::ExitCode;

use oc_ais::eeprom::boot::{boot0_from_elf, boot1_from_elf, boot1_used, updater_from_elf};
use oc_ais::eeprom::image::{app_image, eeprom_image, AppOptions, EepromInputs};
use oc_ais::eeprom::layout::{BODY, BOOT1_CODE_MAX, TRAMP_MAX};
use oc_ais::eeprom::lz4;
use oc_ais::eeprom::verify::{verify, Expected, Level};

const HELP: &str = "\
oc-bootimg build  --tramp T.out --boot1 B.out [--app A.out] [--dump D.bin [--keep-profiles]]
                  [--seq N] [--fw-version N] [--trial] [--allow-legacy-app] -o eeprom.bin
oc-bootimg sizes  --tramp T.out --boot1 B.out [--updater U.out] [--app A.out [--allow-legacy-app]]
oc-bootimg verify dump.bin [--tramp T.out --boot1 B.out]
oc-bootimg lz4    in.bin out.lz4        (LZ4 block of a file, for tests)
Numbers accept 0x prefixes. Nothing is ever sent to the device.";

struct Args {
    pos: Vec<String>,
    opt: HashMap<String, String>,
    flags: Vec<String>,
}

fn parse_args() -> Args {
    const BOOL: [&str; 3] = ["--keep-profiles", "--trial", "--allow-legacy-app"];
    let mut a = Args { pos: Vec::new(), opt: HashMap::new(), flags: Vec::new() };
    let mut it = std::env::args().skip(1);
    while let Some(s) = it.next() {
        if BOOL.contains(&s.as_str()) {
            a.flags.push(s);
        } else if s.starts_with('-') {
            a.opt.insert(s, it.next().unwrap_or_default());
        } else {
            a.pos.push(s);
        }
    }
    a
}

impl Args {
    fn file(&self, k: &str) -> Result<Option<Vec<u8>>, String> {
        self.opt.get(k).map(|p| std::fs::read(p).map_err(|e| format!("{p}: {e}"))).transpose()
    }
    fn need(&self, k: &str) -> Result<Vec<u8>, String> {
        self.file(k)?.ok_or_else(|| format!("missing {k}"))
    }
    fn num(&self, k: &str) -> Result<u32, String> {
        match self.opt.get(k) {
            None => Ok(0),
            Some(v) => match v.strip_prefix("0x") {
                Some(h) => u32::from_str_radix(h, 16),
                None => v.parse(),
            }
            .map_err(|e| format!("{k}: {e}")),
        }
    }
    fn flag(&self, k: &str) -> bool {
        self.flags.iter().any(|f| f == k)
    }
    fn app_opts(&self) -> Result<AppOptions, String> {
        Ok(AppOptions { seq: self.num("--seq")?, fw_version: self.num("--fw-version")?, trial: self.flag("--trial"), allow_legacy_app: self.flag("--allow-legacy-app") })
    }
}

fn pct(a: usize, b: usize) -> f64 {
    100.0 * a as f64 / b as f64
}

fn sizes(a: &Args) -> Result<(), String> {
    let tramp = a.need("--tramp")?;
    let boot1 = a.need("--boot1")?;
    let b0 = boot0_from_elf(&tramp).map_err(|e| e.to_string())?;
    let tl = u32::from_le_bytes(b0[12..16].try_into().unwrap()) as usize;
    println!("trampoline : {tl:6} / {TRAMP_MAX} bytes (boot0 = {} bytes, 2 pages)", tl + 24);
    let used = boot1_used(&boot1).map_err(|e| e.to_string())?;
    println!("boot1      : {used:6} / {BOOT1_CODE_MAX} bytes ({:.1} %)", pct(used, BOOT1_CODE_MAX));
    if let Some(u) = a.file("--updater")? {
        let (segs, entry) = updater_from_elf(&u).map_err(|e| e.to_string())?;
        println!("updater    : {:6} bytes in RAM, entry 0x{entry:08x}", segs.iter().map(|s| s.data.len()).sum::<usize>());
    }
    if let Some(app) = a.file("--app")? {
        let img = app_image(&app, &a.app_opts()?).map_err(|e| e.to_string())?;
        let n = img.body.bytes.len();
        println!("app body   : {n:6} / {} bytes ({:.1} %), {} bytes loaded (LZ4 {:.1} %)", BODY.len(), pct(n, BODY.len()), img.body.raw_len, pct(n, img.body.raw_len as usize));
        match &img.graph {
            Some(g) => println!("graph data : {:6} / {} bytes (8 graph data pages, RAM 0x{:08x}), body margin {} bytes", g.payload.len(), oc_ais::eeprom::graph::CAPACITY, g.addr, BODY.len() - n),
            None => println!("graph data : none (firmware before layout v3), body margin {} bytes", BODY.len() - n),
        }
    }
    Ok(())
}

fn build(a: &Args) -> Result<(), String> {
    let (tramp, boot1) = (a.need("--tramp")?, a.need("--boot1")?);
    let app = a.file("--app")?.map(|e| app_image(&e, &a.app_opts()?).map_err(|e| e.to_string())).transpose()?;
    let dump = a.file("--dump")?;
    if dump.is_none() {
        eprintln!("warning: no --dump: the last page (stock CKT byte at 0xFFFF) is left blank");
    }
    let ee = eeprom_image(&EepromInputs { tramp_elf: &tramp, boot1_elf: &boot1, app: app.as_ref(), dump: dump.as_deref(), keep_profiles: a.flag("--keep-profiles") })
        .map_err(|e| e.to_string())?;
    let out = a.opt.get("-o").ok_or("missing -o")?;
    std::fs::write(out, &ee).map_err(|e| format!("{out}: {e}"))?;
    println!("{out}: {} bytes", ee.len());
    Ok(())
}

fn verify_cmd(a: &Args) -> Result<bool, String> {
    let path = a.pos.get(1).ok_or("missing dump file")?;
    let ee = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let b0 = a.file("--tramp")?.map(|t| boot0_from_elf(&t).map_err(|e| e.to_string())).transpose()?;
    let b1 = a.file("--boot1")?.map(|t| boot1_from_elf(&t).map_err(|e| e.to_string())).transpose()?;
    let r = verify(&ee, &Expected { boot0: b0.as_ref().map(|b| &b[..]), boot1: b1.as_deref() });
    for f in &r.findings {
        println!("{:5} {:10} {}", format!("{:?}", f.level).to_uppercase(), f.area, f.msg);
    }
    println!("=> {:?}", r.outcome);
    Ok(r.worst() < Level::Error)
}

fn run() -> Result<bool, String> {
    let a = parse_args();
    match a.pos.first().map(String::as_str) {
        Some("build") => build(&a).map(|_| true),
        Some("sizes") => sizes(&a).map(|_| true),
        Some("verify") => verify_cmd(&a),
        Some("lz4") if a.pos.len() == 3 => {
            let d = std::fs::read(&a.pos[1]).map_err(|e| e.to_string())?;
            std::fs::write(&a.pos[2], lz4::compress(&d)).map_err(|e| e.to_string())?;
            Ok(true)
        }
        _ => {
            println!("{HELP}");
            Ok(true)
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}
