#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use oc_app::{app, cli};

fn main() -> eframe::Result {
    let env_reduced = std::env::var("OC_REDUCED_MOTION").is_ok_and(|v| v == "1");
    let opts = cli::Options::parse(std::env::args(), env_reduced);
    // One line at every launch: the log exists from the start (support asks for it).
    oc_device::install::log_file::append(&[format!(
        "OpenCompanion {} started on {} {} (data: {}, releases: {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        oc_device::paths::data_dir().display(),
        cfg!(feature = "release-updates"),
    )]);
    // One instance at a time (screenshot runs excepted): a second launch brings the first one forward.
    if opts.shots.is_none() && matches!(oc_app::single_instance::acquire(), oc_app::single_instance::Instance::Other) {
        oc_device::install::log_file::append(&["another OpenCompanion is already running: brought to the front, exiting"]);
        eprintln!("OpenCompanion is already running: its window was brought to the front.");
        return Ok(());
    }
    let run = |renderer: eframe::Renderer, opts: cli::Options| {
        let native = eframe::NativeOptions { renderer, ..app::window::native_options(opts.shots.is_some()) };
        eframe::run_native("OpenCompanion", native, Box::new(move |cc| Ok(Box::new(app::App::new(cc, opts)))))
    };
    // wgpu first; OpenGL only if wgpu cannot start (no adapter, no surface).
    match run(app::window::RENDERER, opts.clone()) {
        Err(e) if app::window::RENDERER == eframe::Renderer::Wgpu => {
            eprintln!("wgpu renderer failed ({e}), falling back to OpenGL");
            run(eframe::Renderer::Glow, opts)
        }
        r => r,
    }
}
