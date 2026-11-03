use std::path::PathBuf;

use eframe::egui::{self, Context, Event, UserData, ViewportCommand};
use oc_device::install::{Chip, Flow};

use super::state::{Pane, Screen};
use super::App;
use crate::i18n::{self, Lang};
use crate::model::{Band, BandType, Target};
use crate::views::{self, install::Stage, state::ThemePref, Cx};

type Setup = Box<dyn Fn(&mut App, &Context)>;

struct Step {
    name: String,
    setup: Setup,
}

enum Phase {
    Setup,
    Wait(u32),
    Requested,
}

pub struct Runner {
    dir: PathBuf,
    steps: Vec<Step>,
    index: usize,
    phase: Phase,
    /// Language of the scenes (`--lang`); French by default.
    lang: Lang,
}

fn eq_profile(app: &mut App, i: usize) {
    app.device.set_active_profile(i);
    app.ui.eq.draft = app.device.profiles()[i].bands.clone();
    app.ui.eq.selected = 0;
}

fn eq_editable(app: &mut App) {
    app.ui.screen = Screen::Equalizer;
    eq_profile(app, 3);
}

fn install(app: &mut App, ctx: &Context, flow: Flow, stage: Stage) {
    install_as(app, ctx, flow, stage, false);
}

fn install_as(app: &mut App, ctx: &Context, flow: Flow, stage: Stage, onboarding: bool) {
    let mut cx = Cx { device: app.device.as_mut(), ui: &mut app.ui };
    views::install::demo_shot(ctx, &mut cx, flow, stage, onboarding);
}

/// The simulated speaker as it is after a restore: the Bose firmware (the app is the onboarding only).
fn bose_speaker(app: &mut App) {
    speaker(app, Some(crate::model::Connection::Stock));
}

fn speaker(app: &mut App, c: Option<crate::model::Connection>) {
    let mut sim = crate::model::SimDevice::new();
    sim.connection = c;
    app.device = Box::new(sim);
}

/// A page of the onboarding, on a simulated speaker in connection state `c`.
fn page(app: &mut App, ctx: &Context, c: Option<crate::model::Connection>, p: views::install::Page) {
    speaker(app, c);
    let mut cx = Cx { device: app.device.as_mut(), ui: &mut app.ui };
    views::install::page_shot(ctx, &mut cx, Flow::First, p);
}

/// Both chips have a newer build (screenshots of the update offer).
fn update_offer() -> oc_device::install::UpdateCheck {
    use oc_device::install::{ChipUpdate, UpdateCheck};
    let u = |chip, from: &str, to: &str| ChipUpdate {
        chip,
        from: Some(from.into()),
        from_stock: false,
        from_probable: false,
        to: Some(to.into()),
        available: true,
    };
    UpdateCheck::Done(vec![u(Chip::Dsp, "0.2.5", "0.2.6"), u(Chip::Usb, "0.5.4", "0.5.5")])
}

fn steps() -> Vec<Step> {
    let mut v: Vec<Step> = Vec::new();
    let mut add = |name: &str, f: Setup| v.push(Step { name: name.into(), setup: f });
    add("home", Box::new(|a, _| a.ui.screen = Screen::Home));
    add("home-muted", Box::new(|a, _| a.device.set_muted(true)));
    // An update is available: the footer line (both chips change).
    add("home-update", Box::new(|a, _| {
        let mut sim = crate::model::SimDevice::new();
        sim.update_check = Some(update_offer());
        a.device = Box::new(sim);
    }));
    // A newer app on GitHub that the newest firmware needs: shown before the firmware offer.
    add("home-app-update", Box::new(|a, _| {
        let mut sim = crate::model::SimDevice::new();
        sim.update_check = Some(update_offer());
        sim.app_update = Some(oc_device::install::AppUpdate {
            version: "1.1.0".into(),
            url: "https://github.com/Uneo7/OpenCompagnon/releases/tag/v1.1.0".into(),
            needed_by_firmware: Some("v1.1.0".into()),
        });
        a.device = Box::new(sim);
    }));
    add("settings-firmware-checking", Box::new(|a, _| {
        let mut sim = crate::model::SimDevice::new();
        sim.update_check = Some(oc_device::install::UpdateCheck::Running);
        a.device = Box::new(sim);
        a.ui.screen = Screen::Settings;
        a.ui.settings.pane = Pane::Firmware;
    }));
    add("home-p2", Box::new(|a, _| eq_profile(a, 1)));
    add("equalizer", Box::new(|a, _| eq_editable(a)));
    add(
        "equalizer-p1",
        Box::new(|a, _| {
            eq_editable(a);
            eq_profile(a, 0);
        }),
    );
    for (name, demo) in [("equalizer-test-prof", "profile-menu"), ("equalizer-test-edit", "edit-menu"), ("equalizer-test-type", "more-menu"), ("equalizer-test-help", "help")] {
        add(
            name,
            Box::new(move |a, _| {
                eq_editable(a);
                a.ui.demo = Some(demo.into());
            }),
        );
    }
    // Duplicate: the target picker (from the locked profile), then the replacement confirmation.
    add(
        "equalizer-duplicate-pick",
        Box::new(|a, _| {
            eq_editable(a);
            eq_profile(a, 0);
            a.ui.demo = Some("dup-menu".into());
        }),
    );
    add(
        "equalizer-duplicate-confirm",
        Box::new(|a, c| {
            eq_editable(a);
            a.apply(c, super::actions::Action::AskDuplicate { from: 0, to: 3 }, None);
        }),
    );
    // Eight bands in the colours of three profile palettes.
    for hue in [0usize, 2, 4] {
        add(
            &format!("equalizer-bands-h{hue}"),
            Box::new(move |a, _| {
                eq_editable(a);
                a.device.set_profile_hue(3, hue);
                let g = a.ui.eq.draft.get_mut(Target::Global);
                for (k, (f, gain)) in [(40.0, 3.0), (90.0, -2.0), (180.0, 4.0), (400.0, -3.0), (900.0, 2.5), (2000.0, -4.0), (5000.0, 3.0), (11000.0, -2.0)]
                    .into_iter()
                    .enumerate()
                {
                    g.push(Band::new(BandType::Peak, f, gain, if k % 2 == 0 { 1.2 } else { 2.0 }));
                }
                a.ui.eq.selected = 2;
            }),
        );
    }
    // README: a gentle, realistic curve (warm bass, slight presence, air).
    add(
        "readme-equalizer",
        Box::new(|a, _| {
            eq_editable(a);
            a.device.rename_profile(3, "Warm");
            let g = a.ui.eq.draft.get_mut(Target::Global);
            g.push(Band::new(BandType::LowShelf, 90.0, 3.0, 0.71));
            g.push(Band::new(BandType::Peak, 320.0, -1.5, 1.0));
            g.push(Band::new(BandType::Peak, 2500.0, 1.5, 0.9));
            g.push(Band::new(BandType::HighShelf, 9000.0, 2.5, 0.71));
            a.ui.eq.selected = 2;
        }),
    );
    add(
        "equalizer-test-adv",
        Box::new(|a, _| {
            eq_editable(a);
            // Advanced is a mode of the profile: the global bands go to each output.
            a.ui.eq.draft.to_advanced();
            a.ui.eq.target = Target::InnerRight;
            a.ui.eq.driver = Target::InnerRight;
            a.ui.eq.selected = 0;
        }),
    );
    add(
        "equalizer-base-eq",
        Box::new(|a, _| {
            eq_editable(a);
            let g = a.ui.eq.draft.get_mut(Target::Global);
            g.push(Band::new(BandType::Peak, 120.0, 4.0, 1.0));
            g.push(Band::new(BandType::HighShelf, 8000.0, -3.0, 0.71));
            a.ui.eq.selected = 0;
        }),
    );
    add(
        "equalizer-base-sub",
        Box::new(|a, _| {
            eq_editable(a);
            // Advanced is a mode of the profile: the global bands go to each output.
            a.ui.eq.draft.to_advanced();
            a.ui.eq.target = Target::Sub;
            a.ui.eq.driver = Target::Sub;
            a.ui.eq.selected = 0;
        }),
    );
    add(
        "equalizer-compare",
        Box::new(|a, _| {
            eq_editable(a);
            a.ui.eq.compare = true;
        }),
    );
    add(
        "equalizer-color",
        Box::new(|a, _| {
            eq_editable(a);
            a.ui.act(super::actions::Action::AskHue(3));
        }),
    );
    for (name, pane) in [
        ("settings", Pane::Status),
        ("settings-sound", Pane::Sound),
        ("settings-outputs", Pane::Outputs),
        ("settings-device", Pane::Device),
        ("settings-firmware", Pane::Firmware),
        ("settings-prefs", Pane::Prefs),
        ("settings-advanced", Pane::Advanced),
        ("settings-legal", Pane::Legal),
    ] {
        add(
            name,
            Box::new(move |a, _| {
                a.ui.screen = Screen::Settings;
                a.ui.settings.pane = pane;
            }),
        );
    }
    add(
        "settings-device-test-select",
        Box::new(|a, _| {
            a.ui.screen = Screen::Settings;
            a.ui.settings.pane = Pane::Device;
            a.ui.demo = Some("standby".into());
        }),
    );
    add("install-confirm", Box::new(|a, c| install(a, c, Flow::First, Stage::Confirm)));
    add("install-run", Box::new(|a, c| install(a, c, Flow::First, Stage::Run(2, 60.0))));
    add("install-crit", Box::new(|a, c| install(a, c, Flow::First, Stage::Run(4, 45.0))));
    add("install-err", Box::new(|a, c| install(a, c, Flow::First, Stage::Fail(5, 45.0))));
    add("onboarding", Box::new(|a, c| install_as(a, c, Flow::First, Stage::Confirm, true)));
    add("onboarding-backup", Box::new(|a, c| install_as(a, c, Flow::First, Stage::Run(0, 50.0), true)));
    add("onboarding-resume", Box::new(|a, c| install_as(a, c, Flow::Update, Stage::Confirm, true)));
    add("install-done", Box::new(|a, c| install(a, c, Flow::First, Stage::Done)));
    add("install-update", Box::new(|a, c| install(a, c, Flow::Update, Stage::Run(1, 40.0))));
    add("install-restore", Box::new(|a, c| install(a, c, Flow::Restore, Stage::Confirm)));
    // A completed restore: the mains power cycle as its last step, then the welcome.
    add(
        "install-restore-done",
        Box::new(|a, c| {
            bose_speaker(a);
            install_as(a, c, Flow::Restore, Stage::Done, true);
        }),
    );
    add("onboarding-welcome", Box::new(|a, c| page(a, c, Some(crate::model::Connection::Stock), views::install::Page::Welcome)));
    add(
        "restore-power-cycle",
        Box::new(|a, c| page(a, c, Some(crate::model::Connection::Stock), views::install::Page::PowerCycle { unplugged: false })),
    );
    add(
        "restore-power-cycle-replug",
        Box::new(|a, c| page(a, c, Some(crate::model::Connection::Absent), views::install::Page::PowerCycle { unplugged: true })),
    );
    add("install-file", Box::new(|a, c| install(a, c, Flow::File(Chip::Dsp, PathBuf::new()), Stage::Run(2, 30.0))));
    add(
        "install-plan",
        Box::new(|a, c| {
            install(a, c, Flow::File(Chip::Usb, PathBuf::new()), Stage::Run(0, 100.0));
            let plan = oc_device::install::Plan { title: "TAS EEPROM install".into(), writes: vec!["U6002 0x0000..0x0011".into()], warnings: Vec::new() };
            let mut cx = Cx { device: a.device.as_mut(), ui: &mut a.ui };
            let Some(p) = views::install::progress(c, &mut cx) else { return };
            let (title, body) = views::install::plan_dialog(&p, &plan);
            a.dialog(title, body, i18n::tr("fw.install.ok"), true, super::actions::Action::ConfirmInstall);
        }),
    );

    let dark: Vec<Step> = std::mem::take(&mut v);
    let mut all = Vec::new();
    for s in dark {
        let setup = std::rc::Rc::new(s.setup);
        let (d, l) = (setup.clone(), setup.clone());
        all.push(Step { name: s.name.clone(), setup: Box::new(move |a, c| d(a, c)) });
        all.push(Step {
            name: format!("{}-clair", s.name),
            setup: Box::new(move |a, c| {
                l(a, c);
                views::prefs::set_theme(c, ThemePref::Light);
                a.ui.light = true;
            }),
        });
        if ["equalizer", "readme-equalizer", "settings-sound", "settings-legal", "install-confirm", "onboarding", "onboarding-welcome", "restore-power-cycle"].contains(&s.name.as_str()) {
            let e = setup.clone();
            all.push(Step {
                name: format!("{}-en", s.name),
                setup: Box::new(move |a, c| {
                    i18n::set_lang(Lang::En);
                    e(a, c);
                }),
            });
        }
    }
    all
}

impl Runner {
    pub fn new(dir: PathBuf, lang: Option<Lang>) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        super::shots_board::write(&dir);
        Self { dir, steps: steps(), index: 0, phase: Phase::Setup, lang: lang.unwrap_or(Lang::Fr) }
    }

    pub fn step(&mut self, ctx: &Context, app: &mut App) -> bool {
        ctx.request_repaint();
        let Some(step) = self.steps.get(self.index) else {
            app.ui.close_confirmed = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
            return true;
        };
        match self.phase {
            Phase::Setup => {
                app.reset();
                views::reset(ctx);
                i18n::set_lang(self.lang);
                views::prefs::set_theme(ctx, ThemePref::Dark);
                (step.setup)(app, ctx);
                self.phase = Phase::Wait(8);
            }
            Phase::Wait(0) => {
                ctx.send_viewport_cmd(ViewportCommand::Screenshot(UserData::new(self.index)));
                self.phase = Phase::Requested;
            }
            Phase::Wait(n) => self.phase = Phase::Wait(n - 1),
            Phase::Requested => {
                let image = ctx.input(|i| {
                    i.raw.events.iter().find_map(|e| match e {
                        Event::Screenshot { image, .. } => Some(image.clone()),
                        _ => None,
                    })
                });
                if let Some(img) = image {
                    let path = self.dir.join(format!("{}.png", step.name));
                    save_png(&path, &img);
                    self.index += 1;
                    self.phase = Phase::Setup;
                }
            }
        }
        false
    }
}

fn save_png(path: &std::path::Path, img: &egui::ColorImage) {
    let [w, h] = img.size;
    let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    if let Err(e) = image::save_buffer_with_format(path, &bytes, w as u32, h as u32, image::ExtendedColorType::Rgba8, image::ImageFormat::Png) {
        eprintln!("screenshot {}: {e}", path.display());
    }
}
