use oc_install::dsp::{self, DspBundle, DspOptions};
use oc_install::restore::{reboot_tas, restore_dsp, restore_tas, DspRestoreInputs, DspRestoreOptions};
use oc_install::tas::{self, OcPort, StockPort, TasOptions, TasTarget};

use super::backup;
use super::check;
use super::versions;
use super::flow::{Chip, Flow, StepId};
use super::host::{Shared, UiHost};
use super::sources::{dsp_version, tas_version, FirmwareSources};
use crate::link::{probe, Conn, Presence};

pub enum RunError {
    /// The user declined a plan: nothing was written by the step that asked.
    Cancelled,
    Failed { msg: String, resumable: bool },
}

pub struct Outcome {
    /// New TAS firmware written: it starts at the next power cycle of the speaker.
    pub power_cycle: bool,
}

type Run<T> = Result<T, RunError>;

fn fail<T>(msg: impl Into<String>, resumable: bool) -> Run<T> {
    Err(RunError::Failed { msg: msg.into(), resumable })
}

fn from_install(e: oc_install::Error) -> RunError {
    match e {
        oc_install::Error::Cancelled => RunError::Cancelled,
        e => RunError::Failed { msg: e.to_string(), resumable: true },
    }
}

/// Opens the speaker again after the USB controller restarted (re-enumeration): up to ~15 s.
fn reopen() -> Run<Conn> {
    let mut last = None;
    for _ in 0..30 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        match open() {
            Ok(c) => return Ok(c),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or(RunError::Failed { msg: "no speaker found on USB".into(), resumable: true }))
}

/// The files of the `.ocfw` at `path` (container checked: magic, chip, CRC-32, app version), extracted; the
/// identity checks of their contents run when the bundle / target is built.
fn from_ocfw(src: &FirmwareSources, path: &std::path::Path, chip: Chip) -> Run<FirmwareSources> {
    let (dir, _version) = super::ocfw::extract_file(path, chip, env!("CARGO_PKG_VERSION")).or_else(|e| fail(e, false))?;
    Ok(FirmwareSources { backups: src.backups.clone(), ..FirmwareSources::from_dir(&dir) })
}

pub(super) fn open() -> Run<Conn> {
    match probe() {
        (_, Some(c)) => Ok(c),
        (Presence::RomDfu { vid, pid }, None) => fail(
            format!(
                "the USB controller is in ROM DFU mode ({vid:04x}:{pid:04x}): an install was interrupted, recovery needed \
                 (opencompanion/docs/recovery.md)"
            ),
            false,
        ),
        (Presence::Inaccessible(e), None) => fail(format!("the speaker's HID interface cannot be opened: {e}"), true),
        _ => fail("no speaker found on USB", true),
    }
}

/// Current versions read from the speaker (with retries, else the worker's last view), target versions from the
/// inputs. `bundle` with its version string (from the ELF; the header only holds the semantic version).
fn set_versions(shared: &Shared, conn: &Conn, flow: &Flow, bundle: Option<(&DspBundle, Option<String>)>, target: Option<&TasTarget>) {
    let known = shared.running();
    let now = versions::read(conn);
    let (dsp, usb) = (now.dsp.or(&known.dsp), now.usb.or(&known.usb));
    let restore = *flow == Flow::Restore;
    shared.update(|p| {
        p.log.push(format!("running: DSP {dsp:?}, USB controller {usb:?}"));
        for v in &mut p.versions {
            let from = match v.chip {
                Chip::Dsp => &dsp,
                Chip::Usb => &usb,
            };
            v.from = from.version();
            v.from_stock = from.is_stock();
            v.from_probable = from.is_probably_stock();
            v.to_stock = restore;
            v.to = match (restore, v.chip) {
                (true, _) => None,
                (false, Chip::Dsp) => bundle.as_ref().map(|(b, full)| full.clone().unwrap_or_else(|| dsp_version(b))),
                (false, Chip::Usb) => target.and_then(tas_version).or_else(|| target.map(|_| "OpenCompanion".into())),
            };
        }
    });
}

fn install_dsp(conn: &Conn, b: &DspBundle, host: &mut UiHost) -> Run<()> {
    let rep = conn.with_dsp(|link| dsp::install(link, b, host, DspOptions { dry_run: false })).map_err(from_install)?;
    if !rep.installed {
        return fail(format!("the DSP firmware is not running after the install (DSP: {})", rep.after), true);
    }
    Ok(())
}

/// Returns true when the new TAS firmware needs a power cycle to start. With our firmware running, pages written
/// are followed by its `REBOOT` (the ROM restarts from U6002, `oc_tas.py reboot`): no power cycle then.
fn install_tas(conn: &Conn, t: &TasTarget, host: &mut UiHost) -> Run<bool> {
    let opts = TasOptions { dry_run: false };
    let rep = match conn {
        Conn::Stock(c) => tas::install(&StockPort::new(c), t, host, opts),
        Conn::Oc(d) => tas::install(&OcPort::new(d), t, host, opts),
    }
    .map_err(from_install)?;
    if !rep.installed {
        return fail("the TAS EEPROM does not hold the image after the install", true);
    }
    Ok(match conn {
        Conn::Oc(d) if rep.pages_written > 0 => reboot_tas(d, host).is_err(),
        _ => rep.pages_written > 0,
    })
}

/// Stock U7001 image of the user's verified backup (never a bundled file): refused without one.
fn stock_dsp(src: &FirmwareSources) -> Run<Vec<u8>> {
    let files = backup::load_stock(&src.backups).or_else(|e| fail(e, false))?;
    files.dsp.map_or_else(|| fail("the stock backup has no U7001 dump", false), Ok)
}

/// Both chips back to the Bose firmware from the stock backup, DSP first. Returns true when the speaker must
/// be power-cycled (the TAS could not be restarted by command).
fn restore(src: &FirmwareSources, conn: &Conn, host: &mut UiHost) -> Run<bool> {
    let files = backup::load_stock(&src.backups).or_else(|e| fail(e, false))?;
    let (Some(image), Some(tas_image)) = (files.dsp, files.tas) else {
        return fail("the stock backup is incomplete", false);
    };
    let crc = crc32fast::hash(&image);
    // Resume after the DSP part was verified (e.g. a garbled version reply ended the earlier run): straight to
    // the TAS, unless the DSP clearly runs something else again.
    let rt = conn.with_dsp(oc_install::dsp::detect);
    let ours = matches!(rt, oc_install::dsp::DspRuntime::OcFirmware { .. } | oc_install::dsp::DspRuntime::Updater { .. } | oc_install::dsp::DspRuntime::Rescue { .. });
    if restored_dsp_marker() == Some(crc) && !ours {
        oc_install::Host::log(host, &format!("DSP: U7001 already restored and verified (CRC-32 0x{crc:08x}, DSP now: {rt}); going on with the USB controller"));
        host.skip_chip(Chip::Dsp);
    } else {
        let (stub, updater) = src.loaders().or_else(|e| fail(e, false))?;
        let img = DspRestoreInputs { image, stub, updater, stock_svm: files.firmware.clone() };
        let rep = conn.with_dsp(|l| restore_dsp(l, &img, host, DspRestoreOptions::default())).map_err(from_install)?;
        if !rep.restored {
            return fail(format!("the Bose DSP firmware is not running after the restore (DSP: {})", rep.after), true);
        }
        set_restored_dsp_marker(Some(crc));
    }
    let opts = TasOptions::default();
    let rep = match conn {
        Conn::Stock(c) => restore_tas(&StockPort::new(c), &tas_image, host, opts),
        Conn::Oc(d) => restore_tas(&OcPort::new(d), &tas_image, host, opts),
    }
    .map_err(from_install)?;
    if !rep.restored {
        return fail("the USB controller EEPROM does not hold the backup after the restore", true);
    }
    set_restored_dsp_marker(None);
    Ok(match conn {
        Conn::Oc(d) if rep.pages_written > 0 => reboot_tas(d, host).is_err(),
        _ => rep.pages_written > 0,
    })
}

/// `<data>/restore-dsp.verified`: CRC-32 of the stock image whose restore to U7001 was verified, until the
/// restore flow completes (lets a resumed restore skip the DSP part).
fn restored_dsp_marker_path() -> std::path::PathBuf {
    crate::paths::data_dir().join("restore-dsp.verified")
}

/// A restore to the Bose firmware was started and its DSP part verified, but the flow did not complete (the
/// USB controller step remains): the app offers to finish it.
pub fn restore_in_progress() -> bool {
    restored_dsp_marker().is_some()
}

fn restored_dsp_marker() -> Option<u32> {
    match std::fs::read_to_string(restored_dsp_marker_path()) {
        Ok(s) => u32::from_str_radix(s.trim(), 16).ok(),
        // Runs of earlier app versions left no marker: their install log tells the same.
        Err(_) => std::fs::read_to_string(super::log_file::path()).ok().and_then(|l| verified_in_log(&l)),
    }
}

/// CRC-32 of the last "R5: U7001 equals the backup" of the log, unless a DSP install wrote U7001 after it.
fn verified_in_log(log: &str) -> Option<u32> {
    let mut crc = None;
    for line in log.lines() {
        if let Some(at) = line.find("R5: U7001 equals the backup (64 KB, CRC-32 0x") {
            let hex = &line[at + "R5: U7001 equals the backup (64 KB, CRC-32 0x".len()..];
            crc = u32::from_str_radix(hex.get(..8)?, 16).ok();
        } else if line.contains("M4:") || line.contains("M2:") || line.contains("[Restore] Done") {
            crc = None; // U7001 written again since, or the restore completed (a failed flow that wrote nothing does not count)
        }
    }
    crc
}

fn set_restored_dsp_marker(crc: Option<u32>) {
    let p = restored_dsp_marker_path();
    match crc {
        Some(c) => {
            if let Some(d) = p.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(&p, format!("{c:08x}"));
        }
        None => {
            let _ = std::fs::remove_file(&p);
        }
    }
}

/// Production builds: the release found by the update check and not downloaded yet is downloaded (step
/// "Download", real progress), checked and stored; its folder becomes the installer's files.
#[cfg(feature = "github")]
fn prepare_release(src: &FirmwareSources, shared: &Shared, host: &mut UiHost) -> Run<FirmwareSources> {
    let Some(av) = shared.release().pending.clone() else { return Ok(src.clone()) };
    host.enter(StepId::Download);
    shared.update(|p| p.log.push(format!("downloading {} ({} bytes, release {})", av.zip.name, av.zip.size, av.tag)));
    let mut last = 0u8;
    let s = download_release(shared, &av, |pct| {
        if pct != last {
            last = pct;
            shared.update(|p| p.step_percent(pct));
        }
    })
    .or_else(|e| fail(e, true))?;
    shared.update(|p| p.log.push(format!("release {} verified: {}", av.tag, s.dsp_app.parent().unwrap_or(&s.dsp_app).display())));
    Ok(s)
}

/// Downloads, checks and stores `av`; its folder becomes the installer's files.
#[cfg(feature = "github")]
pub(super) fn download_release(shared: &Shared, av: &super::release::github::Available, mut percent: impl FnMut(u8)) -> Result<FirmwareSources, String> {
    use super::release::{github, store};
    let zip = github::download(&av.zip, |done, total| percent((done * 100 / total).min(99) as u8))
        .map_err(|e| format!("download of the firmware: {e}"))?;
    let dir = store::install(&crate::paths::firmware_dir(), &av.tag, &zip, av.zip_sha256.as_deref(), &av.manifest)?;
    let s = FirmwareSources::from_dir(&dir);
    shared.set_sources(s.clone());
    let mut rel = shared.release();
    rel.pending = None;
    rel.status = Some(super::host::ReleaseStatus::InUse { tag: av.tag.clone() });
    Ok(s)
}

#[cfg(test)]
mod marker_tests {
    #[test]
    fn a_verified_restore_is_found_in_the_log() {
        let log = ["x", "2026-11-02 R5: U7001 equals the backup (64 KB, CRC-32 0x1a2b3c4d).", "2026-11-02 DSP after reboot: no answer"]
            .join("\n");
        assert_eq!(super::verified_in_log(&log), Some(0x1a2b_3c4d));
        let later = [log.as_str(), "M4: writing the image body"].join("\n");
        assert_eq!(super::verified_in_log(&later), None);
        assert_eq!(super::verified_in_log("nothing"), None);
        let failed = [log.as_str(), "[Update] Running step inst.step.prep.update", "ERROR: the DSP does not answer"].join("\n");
        assert_eq!(super::verified_in_log(&failed), Some(0x1a2b_3c4d), "a flow that wrote nothing keeps the restore");
        let done = [log.as_str(), "2026-11-02 [Restore] Done step -"].join("\n");
        assert_eq!(super::verified_in_log(&done), None, "a completed restore is not in progress");
    }
}

/// Nothing to install: says why (the GitHub lookup when releases are used, else the missing file).
fn no_firmware(shared: &Shared, src: &FirmwareSources) -> String {
    match shared.release().status.clone() {
        Some(s) => format!("{}: {s}", super::NO_FIRMWARE),
        None => format!("{}: {} not found", super::NO_FIRMWARE, src.dsp_app.display()),
    }
}

#[cfg(not(feature = "github"))]
fn prepare_release(src: &FirmwareSources, _shared: &Shared, _host: &mut UiHost) -> Run<FirmwareSources> {
    Ok(src.clone())
}

pub fn run(flow: &Flow, src: &FirmwareSources, shared: &Shared, host: &mut UiHost) -> Run<Outcome> {
    if matches!(flow, Flow::First | Flow::Update) {
        set_restored_dsp_marker(None); // OpenCompanion goes back on the DSP
    }
    let fresh;
    let src = if matches!(flow, Flow::First | Flow::Update) {
        fresh = prepare_release(src, shared, host)?;
        if !fresh.dsp_app.is_file() {
            return fail(no_firmware(shared, &fresh), false);
        }
        &fresh
    } else {
        src
    };
    let inputs = |dsp: bool, tas: bool, app: Option<&std::path::Path>, img: Option<&std::path::Path>| -> Run<(Option<DspBundle>, Option<TasTarget>)> {
        let b = if dsp { Some(src.dsp_bundle(app, stock_dsp(src)?).or_else(|e| fail(e, false))?) } else { None };
        let t = if tas { Some(src.tas_target(img).or_else(|e| fail(e, false))?) } else { None };
        Ok((b, t))
    };
    match flow {
        Flow::First => {
            // The first install refuses to start before a verified backup of THIS speaker exists (read from it):
            // the backup is a step of its own, and nothing is written before it succeeds.
            host.enter(StepId::Backup);
            let conn = open()?;
            let entry = backup::ensure(src, &conn, host).or_else(|e| fail(format!("backup: {e}"), true))?;
            shared.update(|p| p.log.push(format!("backup: {}", entry.entry.dir.display())));
            host.enter(StepId::Prep);
            let (b, t) = inputs(true, true, None, None)?;
            let (b, t) = (b.expect("dsp"), t.expect("tas"));
            set_versions(shared, &conn, flow, Some((&b, src.dsp_app_version(None))), Some(&t));
            install_dsp(&conn, &b, host)?;
            let power_cycle = install_tas(&conn, &t, host)?;
            Ok(Outcome { power_cycle })
        }
        Flow::Update => {
            host.enter(StepId::PrepUpdate);
            let (b, t) = inputs(true, true, None, None)?;
            let (b, t) = (b.expect("dsp"), t.expect("tas"));
            let conn = open()?;
            set_versions(shared, &conn, flow, Some((&b, src.dsp_app_version(None))), Some(&t));
            // A DSP that does not answer properly (e.g. a stale relay reply of TAS 0.4.9 with the stock DSP): the
            // USB controller, which does not need it, first; then the DSP through the new relay.
            if let oc_install::dsp::DspRuntime::Silent(e) = conn.with_dsp(oc_install::dsp::detect) {
                oc_install::Host::log(host, &format!("DSP does not answer properly ({e}): updating the USB controller first"));
                let power_cycle = install_tas(&conn, &t, host)?;
                drop(conn);
                let conn = reopen()?;
                if let oc_install::dsp::DspRuntime::Silent(e) = conn.with_dsp(oc_install::dsp::detect) {
                    return fail(
                        format!("the DSP still does not answer after the USB controller update ({e}): unplug the speaker from the mains for 10 s, then resume"),
                        true,
                    );
                }
                install_dsp(&conn, &b, host)?;
                return Ok(Outcome { power_cycle });
            }
            // The DSP is left alone when it already runs this build (same version and build stamp).
            let running = shared.lock().versions.iter().find(|v| v.chip == Chip::Dsp).and_then(|v| v.from.clone());
            if check::dsp_differs(running.as_deref(), src.dsp_app_version(None).as_deref()) {
                install_dsp(&conn, &b, host)?;
            } else {
                shared.update(|p| {
                    p.log.push("DSP: this build already runs, skipped".into());
                    p.skip_chip(Chip::Dsp);
                });
            }
            let power_cycle = install_tas(&conn, &t, host)?;
            Ok(Outcome { power_cycle })
        }
        Flow::File(Chip::Dsp, path) => {
            host.enter(StepId::FileCheck);
            let fsrc = from_ocfw(src, path, Chip::Dsp)?;
            let b = fsrc.dsp_bundle(None, stock_dsp(src)?).or_else(|e| fail(e, false))?;
            let conn = open()?;
            set_versions(shared, &conn, flow, Some((&b, fsrc.dsp_app_version(None))), None);
            install_dsp(&conn, &b, host)?;
            Ok(Outcome { power_cycle: false })
        }
        Flow::File(Chip::Usb, path) => {
            host.enter(StepId::FileCheck);
            let fsrc = from_ocfw(src, path, Chip::Usb)?;
            let t = fsrc.tas_target(None).or_else(|e| fail(e, false))?;
            let conn = open()?;
            set_versions(shared, &conn, flow, None, Some(&t));
            let power_cycle = install_tas(&conn, &t, host)?;
            Ok(Outcome { power_cycle })
        }
        Flow::Restore => {
            host.enter(StepId::RestoreCheck);
            let Some((date, dir)) = backup::latest(&src.backups) else {
                return fail(format!("no verified backup of the Bose firmware in {}", src.backups.display()), false);
            };
            shared.update(|p| p.log.push(format!("backup of {date}: {}", dir.display())));
            let conn = open()?;
            set_versions(shared, &conn, flow, None, None);
            let power_cycle = restore(src, &conn, host)?;
            Ok(Outcome { power_cycle })
        }
    }
}
