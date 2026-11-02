mod sim;

use std::path::PathBuf;
use std::rc::Rc;

use oc_install::backup::{self, ensure_stock_backup, list, load, read_dsp, stock_backup_status};
use oc_install::dsp::{install, loader_digests, DspBundle, DspOptions, Loaders, StockRef};
use oc_install::restore::{restore_dsp, restore_tas, DspRestoreInputs, DspRestoreOptions};
use oc_install::tas::{self, OcPort, StockPort, TasOptions};
use oc_install::Error;
use oc_protocol::oc_hid::OcRelay;
use sim::dsp::DspRun;
use sim::fixture;
use sim::tas::TasRun;
use sim::{BoseLink, Fault, Known, OcSim, Shared, Speaker, TestHost, ALL_FAULTS};

fn known(b: &DspBundle) -> Rc<Known> {
    let mut rng = fixture::Rng::new(7);
    Rc::new(Known { dsp: b.clone(), tas_bose: fixture::tas_bose(&mut rng), tas_oc: vec![fixture::tas_target(1, 5000, 0xF0)] })
}

fn root(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("oc-backup-it-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

fn ensure(sp: &Shared, b: &DspBundle, dir: &std::path::Path) -> oc_install::Result<backup::StockBackup> {
    let digests = loader_digests(&b.stock).unwrap();
    let loaders = Loaders { stock: StockRef::Digests(&digests), stub: &b.stub, updater: &b.updater };
    let mut host = TestHost::new(sp, true);
    let link = BoseLink(sp.clone());
    let run = sp.borrow().tas.run;
    match run {
        TasRun::Bose => {}
        _ => {
            let oc = OcSim(sp.clone());
            return ensure_stock_backup(dir, &OcPort::new(&oc), &OcRelay::new(&oc), &loaders, &mut host);
        }
    }
    ensure_stock_backup(dir, &StockPort::new(&link), &link, &loaders, &mut host)
}

#[test]
fn automatic_stock_backup_reads_the_speaker_once() {
    let b = fixture::bundle(1, 1);
    let sp = Speaker::new(known(&b));
    let dir = root("auto");
    assert!(stock_backup_status(&dir).is_none());
    let first = ensure(&sp, &b, &dir).unwrap();
    assert!(first.created && first.entry.is_stock());
    assert_eq!(sp.borrow().ee_writes, 0, "a backup writes nothing");
    assert_eq!(sp.borrow().dsp.run, DspRun::Stock, "the DSP is back on the Bose firmware");
    let files = load(&first.entry.dir).unwrap();
    assert_eq!(files.dsp.unwrap(), b.stock, "U7001 read from the speaker");
    assert_eq!(files.tas.unwrap(), sp.borrow().tas.ee, "U6002 read from the speaker");
    let ops = sp.borrow().ops;
    let again = ensure(&sp, &b, &dir).unwrap();
    assert!(!again.created && again.entry == first.entry);
    assert!(sp.borrow().ops - ops < 400, "only U6002 is read to recognise the speaker");
    assert_eq!(list(&dir).len(), 1);
    assert_eq!(stock_backup_status(&dir).unwrap().date(), &first.entry.stamp[..10]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_a_stock_backup_an_installed_speaker_is_refused() {
    let b = fixture::bundle(1, 1);
    let sp = Speaker::new(known(&b));
    let mut host = TestHost::new(&sp, true);
    install(&BoseLink(sp.clone()), &b, &mut host, DspOptions::default()).unwrap();
    let dir = root("refused");
    assert!(matches!(ensure(&sp, &b, &dir), Err(Error::Refused(_))));
    assert!(list(&dir).is_empty());
}

#[test]
fn dsp_backup_from_our_firmware_uses_rsc_and_writes_nothing() {
    let b = fixture::bundle(1, 1);
    let sp = Speaker::new(known(&b));
    let mut host = TestHost::new(&sp, true);
    install(&BoseLink(sp.clone()), &b, &mut host, DspOptions::default()).unwrap();
    let (w, ee) = (sp.borrow().ee_writes, sp.borrow().dsp.ee.clone());
    let dump = read_dsp(&BoseLink(sp.clone()), &b.loaders(), &mut host).unwrap();
    assert_eq!(dump.bytes, ee);
    assert_eq!(sp.borrow().ee_writes, w);
    assert_eq!(sp.borrow().dsp.run, DspRun::Oc);
    // Without RAM retention RSC fails, and a backup never falls back to UPD.
    sp.borrow_mut().dsp.ram_retained = false;
    assert!(read_dsp(&BoseLink(sp.clone()), &b.loaders(), &mut host).is_err());
    assert_eq!(sp.borrow().ee_writes, w);
}

#[test]
fn an_interrupted_backup_never_writes_and_saves_nothing() {
    let b = fixture::bundle(1, 1);
    let total = {
        let sp = Speaker::new(known(&b));
        let dir = root("count");
        ensure(&sp, &b, &dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let n = sp.borrow().ops;
        n
    };
    for kind in ALL_FAULTS {
        for at in (0..total).step_by(7) {
            let sp = Speaker::new(known(&b));
            let dir = root(&format!("int-{at}"));
            sp.borrow_mut().fault = Some(Fault { at, kind });
            let r = ensure(&sp, &b, &dir);
            let mut s = sp.borrow_mut();
            assert_eq!(s.ee_writes, 0, "{kind:?} at {at}");
            assert_eq!(r.is_ok(), !list(&dir).is_empty(), "a failed backup leaves no complete folder");
            s.reconnect();
            s.power_cycle();
            assert_eq!((s.dsp.run, s.tas.run), (DspRun::Stock, TasRun::Bose));
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

#[test]
fn full_cycle_backup_install_restore_from_the_saved_files() {
    let b = fixture::bundle(1, 1);
    let sp = Speaker::new(known(&b));
    let dir = root("cycle");
    let entry = ensure(&sp, &b, &dir).unwrap().entry;
    let stock_tas = sp.borrow().tas.ee.clone();
    let mut host = TestHost::new(&sp, true);
    install(&BoseLink(sp.clone()), &b, &mut host, DspOptions::default()).unwrap();
    let t = sp.borrow().known.tas_oc[0].clone();
    tas::install(&StockPort::new(&BoseLink(sp.clone())), &t, &mut host, TasOptions::default()).unwrap();
    sp.borrow_mut().power_cycle();
    assert_eq!(sp.borrow().tas.run, TasRun::Oc(0));
    let files = load(&entry.dir).unwrap();
    let img = DspRestoreInputs { image: files.dsp.unwrap(), stub: b.stub.clone(), updater: b.updater.clone() };
    let oc = OcSim(sp.clone());
    restore_dsp(&OcRelay::new(&oc), &img, &mut host, DspRestoreOptions::default()).unwrap();
    restore_tas(&OcPort::new(&oc), &files.tas.unwrap(), &mut host, TasOptions::default()).unwrap();
    sp.borrow_mut().power_cycle();
    let s = sp.borrow();
    assert_eq!((s.dsp.run, s.tas.run), (DspRun::Stock, TasRun::Bose));
    assert_eq!(s.dsp.ee, b.stock);
    assert_eq!(s.tas.ee, stock_tas);
    drop(s);
    let _ = std::fs::remove_dir_all(&dir);
}
