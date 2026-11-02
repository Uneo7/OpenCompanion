#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use oc_app::{app, cli};

fn main() -> eframe::Result {
    let env_reduced = std::env::var("OC_REDUCED_MOTION").is_ok_and(|v| v == "1");
    let opts = cli::Options::parse(std::env::args(), env_reduced);
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
