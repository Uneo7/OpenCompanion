use eframe::egui::Context;

use super::{actions::Action, state::UiState, App};
use crate::model::{eq::MAX_BANDS, BandType, Base, Device, ProfileBands, SimDevice, Target};
use crate::widgets::{dialog::Body, eq_graph::Change};

fn app() -> (App, Context) {
    let device: Box<dyn Device> = Box::new(SimDevice::new());
    let ui = UiState::new(device.as_ref());
    (App { device, ui, shots: None, _tray: None }, Context::default())
}

/// The app on profile 2 ("Bureau", Office): profile 1, "BoseÂ® d'origine" (BoseÂ® original), is read-only.
fn editable() -> (App, Context) {
    let (mut app, ctx) = app();
    run(&mut app, &ctx, Action::ActivateProfile(1));
    (app, ctx)
}

fn with_connection(c: Option<crate::model::Connection>) -> (App, Context) {
    let mut sim = SimDevice::new();
    sim.connection = c;
    let device: Box<dyn Device> = Box::new(sim);
    let ui = UiState::new(device.as_ref());
    (App { device, ui, shots: None, _tray: None }, Context::default())
}

fn run(app: &mut App, ctx: &Context, a: Action) -> bool {
    app.apply(ctx, a, None)
}

fn add_bands(app: &mut App, ctx: &Context, n: usize) {
    for k in 0..n {
        run(app, ctx, Action::AddBand(Some((200.0 * (k + 1) as f64, 1.0))));
    }
}

#[test]
fn starts_on_profile_1_on_the_bose_base() {
    let (app, _) = app();
    assert_eq!(app.device.active_profile(), 0);
    assert_eq!(app.ui.eq.draft, ProfileBands::bose());
}

#[test]
fn edits_mark_dirty_and_save_writes_the_profile() {
    let (mut app, ctx) = editable();
    assert!(!app.ui.eq.dirty);
    add_bands(&mut app, &ctx, 1);
    run(&mut app, &ctx, Action::SetBandType(0, BandType::LowShelf));
    assert!(app.ui.eq.dirty);
    assert_ne!(app.device.profiles()[1].bands, app.ui.eq.draft);
    app.save(&ctx);
    assert!(!app.ui.eq.dirty);
    assert_eq!(app.device.profiles()[1].bands, app.ui.eq.draft);
}

#[test]
fn the_bose_profile_is_read_only() {
    let (mut app, ctx) = app();
    assert!(app.device.profiles()[0].locked);
    add_bands(&mut app, &ctx, 2);
    run(&mut app, &ctx, Action::Flatten(0));
    app.save(&ctx);
    assert!(!app.ui.eq.dirty);
    assert_eq!(app.ui.eq.draft, ProfileBands::bose());
    let name = Body::Field { value: "X".into(), max_len: 24 };
    app.apply(&ctx, Action::Rename(0), Some(&name));
    assert_eq!(app.device.profiles()[0].name, "Bose d'origine");
    // The device refuses it too.
    let mut b = ProfileBands::bose();
    b.get_mut(Target::Global).push(crate::model::Band::new(BandType::Peak, 100.0, 3.0, 1.0));
    assert!(!app.device.store_profile(0, &b));
}

#[test]
fn switching_profile_saves_pending_changes_first() {
    let (mut app, ctx) = editable();
    add_bands(&mut app, &ctx, 2);
    let edited = app.ui.eq.draft.clone();
    run(&mut app, &ctx, Action::ActivateProfile(2));
    assert_eq!(app.device.active_profile(), 2);
    assert_eq!(app.device.profiles()[1].bands, edited);
    assert_eq!(app.ui.eq.draft, app.device.profiles()[2].bands);
    assert_eq!(app.ui.eq.selected, 0);
}

#[test]
fn bands_are_limited() {
    let (mut app, ctx) = editable();
    for _ in 0..20 {
        run(&mut app, &ctx, Action::AddBand(Some((25.0, 3.0))));
    }
    assert_eq!(app.ui.eq.draft.get(Target::Global).len(), MAX_BANDS);
    assert_eq!(app.ui.eq.draft.get(Target::Global)[6].kind, BandType::HighPass);
    assert!(app.ui.eq.dirty);
}

#[test]
fn delete_keeps_a_valid_selection() {
    let (mut app, ctx) = editable();
    add_bands(&mut app, &ctx, 6);
    app.ui.eq.selected = 5;
    run(&mut app, &ctx, Action::DeleteBand);
    assert_eq!(app.ui.eq.draft.get(Target::Global).len(), 5);
    assert_eq!(app.ui.eq.selected, 4);
}

#[test]
fn linked_drivers_follow_each_other() {
    let (mut app, ctx) = editable();
    run(&mut app, &ctx, Action::SetTarget(Target::OuterLeft));
    run(&mut app, &ctx, Action::AddBand(Some((3000.0, -2.0))));
    assert_eq!(app.ui.eq.draft.get(Target::OuterRight), app.ui.eq.draft.get(Target::OuterLeft));
    run(&mut app, &ctx, Action::SetLink(false));
    run(&mut app, &ctx, Action::DeleteBand);
    assert_eq!(app.ui.eq.draft.get(Target::OuterLeft).len() + 1, app.ui.eq.draft.get(Target::OuterRight).len());
    // Selecting does not count as an edit.
    app.ui.eq.dirty = false;
    run(&mut app, &ctx, Action::Edited(Change::Select));
    assert!(!app.ui.eq.dirty);
}

#[test]
fn dialogs_validate_their_input() {
    let (mut app, ctx) = app();
    let empty = Body::Field { value: "  ".into(), max_len: 24 };
    assert!(!app.apply(&ctx, Action::Rename(2), Some(&empty)));
    let name = Body::Field { value: "SoirÃ©e".into(), max_len: 24 };
    assert!(app.apply(&ctx, Action::Rename(2), Some(&name)));
    assert_eq!(app.device.profiles()[2].name, "SoirÃ©e");
    let wrong = Body::Confirm { text: String::new(), placeholder: String::new(), value: "bosÃ©".into() };
    assert!(!app.apply(&ctx, Action::Factory, Some(&wrong)));
    let right = Body::Confirm { text: String::new(), placeholder: String::new(), value: " bose ".into() };
    assert!(app.apply(&ctx, Action::Factory, Some(&right)));
    let hue = Body::Hues { hues: Vec::new(), selected: 7, taken: Vec::new() };
    app.apply(&ctx, Action::SetHue(1), Some(&hue));
    assert_eq!(app.device.profiles()[1].hue, 7);
}

#[test]
fn flatten_changes_the_draft() {
    let (mut app, ctx) = editable();
    add_bands(&mut app, &ctx, 3);
    // "Reset to flat": flat EQ, on the Bose base like every profile.
    run(&mut app, &ctx, Action::Flatten(1));
    assert_eq!(app.ui.eq.draft, ProfileBands::bose());
    assert!(app.ui.eq.dirty);
    app.save(&ctx);
    assert_eq!(app.device.profiles()[1].base(), Base::Bose);
}

#[test]
fn volume_mute_and_close_dialog() {
    let (mut app, ctx) = app();
    run(&mut app, &ctx, Action::SetVolume(130));
    assert_eq!(app.device.volume(), 100);
    run(&mut app, &ctx, Action::ToggleMute);
    assert!(app.device.muted());
    // Closing asks nothing (no dialog) when no install is writing.
    run(&mut app, &ctx, Action::AskClose);
    assert!(app.ui.dialog.is_none());
}

#[test]
fn the_eq_mode_switches_keep_or_flatten_the_sound() {
    let (mut app, ctx) = editable();
    add_bands(&mut app, &ctx, 2);
    let global = app.ui.eq.draft.get(Target::Global).clone();
    run(&mut app, &ctx, Action::SetAdvanced(true));
    assert!(app.ui.eq.draft.advanced && app.ui.eq.draft.get(Target::Global).is_empty());
    assert_eq!(app.ui.eq.draft.get(Target::Sub), &global);
    assert!(app.ui.eq.target.is_advanced());
    // Advanced â†’ Simple asks first.
    run(&mut app, &ctx, Action::AskSimple);
    assert!(app.ui.dialog.is_some());
    run(&mut app, &ctx, Action::SetAdvanced(false));
    assert!(!app.ui.eq.draft.advanced && app.ui.eq.draft.get(Target::Sub).is_empty());
    assert_eq!(app.ui.eq.target, Target::Global);
    app.save(&ctx);
    assert!(!app.device.profiles()[1].bands.advanced);
}

#[test]
fn duplicate_copies_into_another_slot_and_edits_the_copy() {
    let (mut app, ctx) = app();
    let bose = app.device.profiles()[0].bands.clone();
    // Never into the locked slot.
    run(&mut app, &ctx, Action::Duplicate { from: 2, to: 0 });
    assert_eq!(app.device.active_profile(), 0);
    run(&mut app, &ctx, Action::Duplicate { from: 0, to: 2 });
    let p = &app.device.profiles()[2];
    assert!(p.name.starts_with("Bose (") && !p.locked && p.bands == bose);
    assert_eq!(app.device.active_profile(), 2);
    assert!(!app.ui.eq.dirty);
    // The copy has a colour no other profile uses.
    let hues: Vec<_> = app.device.profiles().iter().map(|p| p.hue).collect();
    assert_eq!(hues.iter().filter(|h| **h == p.hue).count(), 1);
}

#[test]
fn duplicate_always_asks_for_the_target_then_confirms_the_replacement() {
    use crate::views::Cx;
    use crate::widgets::menu::Entry;
    let (mut app, ctx) = app();
    // From the locked profile: the other profiles only, the locked one never (nor the source).
    let targets = |app: &mut App, from: usize| {
        let cx = Cx { device: app.device.as_mut(), ui: &mut app.ui };
        crate::views::equalizer::menus::duplicate_entries(&cx, from)
            .into_iter()
            .filter_map(|e| match e {
                Entry::Item { action: Action::AskDuplicate { to, .. }, .. } => Some(to),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(targets(&mut app, 0), vec![1, 2, 3]);
    assert_eq!(targets(&mut app, 2), vec![1, 3]);
    // An occupied slot: a destructive confirmation first, nothing written before it.
    let before = app.device.profiles()[3].clone();
    assert!(run(&mut app, &ctx, Action::AskDuplicate { from: 1, to: 3 }));
    let d = app.ui.dialog.as_ref().expect("confirmation");
    assert!(d.danger && d.action == Action::Duplicate { from: 1, to: 3 });
    assert_eq!(app.device.profiles()[3], before);
    run(&mut app, &ctx, Action::Duplicate { from: 1, to: 3 });
    assert_eq!(app.device.active_profile(), 3);
    assert_eq!(app.device.profiles()[3].bands, app.device.profiles()[1].bands);
    assert!(app.device.profiles()[3].name.ends_with(')'));
    // The locked slot is never a target, even asked directly.
    run(&mut app, &ctx, Action::AskDuplicate { from: 1, to: 0 });
    run(&mut app, &ctx, Action::Duplicate { from: 1, to: 0 });
    assert!(app.device.profiles()[0].locked && app.device.active_profile() == 3);
}

#[test]
fn copy_names_fit_the_speaker() {
    let p = crate::model::Profile { name: "Un nom assez long".into(), hue: 1, bands: Default::default(), locked: false };
    let n = super::dispatch::duplicate_name(&p, 1);
    assert!(n.len() <= oc_device::params::NAME_LEN, "{n}");
    assert!(n.ends_with(')'));
}

#[test]
fn edits_are_auto_saved_after_a_pause_only() {
    let (mut app, ctx) = editable();
    add_bands(&mut app, &ctx, 1);
    // Right after the edit: not yet (debounced).
    app.auto_save(&ctx);
    assert!(app.ui.eq.dirty);
    assert_ne!(app.device.profiles()[1].bands, app.ui.eq.draft);
    // The pause has passed.
    app.ui.eq.edited_at -= 2.0;
    app.auto_save(&ctx);
    assert!(!app.ui.eq.dirty);
    assert_eq!(app.device.profiles()[1].bands, app.ui.eq.draft);
}

#[test]
fn onboarding_takes_over_until_the_speaker_runs_opencompanion() {
    use crate::model::Connection;
    use crate::views::{install, Cx};
    use oc_device::install::Flow;
    let cases = [
        (Some(Connection::Stock), Some(Flow::First)),
        (Some(Connection::DspNotReady("Bose".into())), Some(Flow::Update)),
        (Some(Connection::Recovery), None),
        (Some(Connection::Absent), None),
        (Some(Connection::Ready), None),
        (None, None),
    ];
    for (conn, flow) in cases {
        let (mut app, ctx) = with_connection(conn.clone());
        let mut cx = Cx { device: app.device.as_mut(), ui: &mut app.ui };
        let shown = install::visible(&ctx, &mut cx);
        assert_eq!(shown, flow.is_some(), "{conn:?}");
        assert_eq!(install::onboarding(&ctx), flow.is_some(), "{conn:?}");
        if let Some(f) = flow {
            assert_eq!(install::progress(&ctx, &mut cx).map(|p| p.flow), Some(f), "{conn:?}");
            // A tab click does not leave the onboarding.
            install::leave(&ctx, &mut cx);
            assert!(install::onboarding(&ctx));
        }
    }
}

#[test]
fn settings_of_the_locked_profile_write_nothing() {
    let (mut app, ctx) = with_connection(None);
    assert!(app.device.profiles()[app.device.active_profile()].locked);
    let before = app.device.settings().clone();
    assert!(run(&mut app, &ctx, Action::SetLoudness(!before.loudness)));
    let mut s = before.clone();
    s.aux_gain = 6.0;
    s.trims[1] = 2.0;
    app.device.set_settings(s);
    assert_eq!(app.device.settings(), &before);
    // The bass module level is a device setting: still editable.
    let mut s = before.clone();
    s.sub_level = 3.0;
    app.device.set_settings(s);
    assert_eq!(app.device.settings().sub_level, 3.0);
}

#[test]
fn a_restart_asks_first_then_runs_until_the_speaker_answers() {
    use crate::model::FirmwarePart;
    let (mut app, ctx) = with_connection(None);
    assert!(run(&mut app, &ctx, Action::AskRestart(FirmwarePart::Dsp)));
    assert!(app.ui.dialog.is_some());
    assert_eq!(app.device.restarting(), None);
    assert!(run(&mut app, &ctx, Action::Restart(FirmwarePart::Dsp)));
    assert_eq!(app.device.restarting(), Some(FirmwarePart::Dsp));
    // A second restart while one runs is ignored.
    app.device.restart(FirmwarePart::Usb);
    assert_eq!(app.device.restarting(), Some(FirmwarePart::Dsp));
    app.device.tick(10.0);
    assert_eq!(app.device.restarting(), None);
    assert!(app.device.take_notice().is_some());
}

#[test]
fn a_bose_speaker_only_ever_gets_the_onboarding() {
    use crate::model::Connection;
    use crate::views::{install, Cx};
    use oc_device::install::Flow;
    let flow = |app: &mut App, ctx: &Context| {
        let mut cx = Cx { device: app.device.as_mut(), ui: &mut app.ui };
        install::progress(ctx, &mut cx).map(|p| p.flow)
    };
    let shown = |app: &mut App, ctx: &Context| {
        let mut cx = Cx { device: app.device.as_mut(), ui: &mut app.ui };
        install::visible(ctx, &mut cx) && install::onboarding(ctx)
    };

    use install::Page;
    let swap = |app: &mut App, c: Connection| {
        let mut sim = SimDevice::new();
        sim.connection = Some(c);
        app.device = Box::new(sim);
    };

    // At launch: the welcome page, then the installer.
    let (mut app, ctx) = with_connection(Some(Connection::Stock));
    assert!(shown(&mut app, &ctx));
    assert_eq!(install::page(&ctx), Some(Page::Welcome));
    assert_eq!(flow(&mut app, &ctx), Some(Flow::First));

    // Replugged: OpenCompanion first (the app), then the Bose firmware (the onboarding only).
    let (mut app, ctx) = with_connection(Some(Connection::Ready));
    assert!(!shown(&mut app, &ctx));
    let mut sim = SimDevice::new();
    sim.connection = Some(Connection::Stock);
    app.device = Box::new(sim);
    assert!(shown(&mut app, &ctx));
    assert_eq!(install::page(&ctx), Some(Page::Welcome));

    // A restore run from Settings, completed: its Done screen shows without tabs; Finish leads to the power cycle.
    let (mut app, ctx) = with_connection(Some(Connection::Ready));
    {
        let mut cx = Cx { device: app.device.as_mut(), ui: &mut app.ui };
        install::demo_shot(&ctx, &mut cx, Flow::Restore, install::Stage::Done, false);
    }
    assert!(!install::onboarding(&ctx));
    let mut sim = SimDevice::new();
    sim.connection = Some(Connection::Stock);
    app.device = Box::new(sim);
    assert!(shown(&mut app, &ctx), "the restore's own screen, as the onboarding");
    assert_eq!(flow(&mut app, &ctx), Some(Flow::Restore));
    {
        let mut cx = Cx { device: app.device.as_mut(), ui: &mut app.ui };
        install::leave(&ctx, &mut cx);
        install::press(&ctx, &mut cx, install::Event::Finish);
    }
    // Finish: the power cycle page, following the speaker out and back, then the welcome.
    assert!(shown(&mut app, &ctx));
    assert_eq!(install::page(&ctx), Some(Page::PowerCycle { unplugged: false }));
    swap(&mut app, Connection::Absent);
    assert!(shown(&mut app, &ctx));
    assert_eq!(install::page(&ctx), Some(Page::PowerCycle { unplugged: true }));
    swap(&mut app, Connection::Stock);
    assert!(shown(&mut app, &ctx), "after the power cycle: the welcome, never the app");
    assert_eq!(install::page(&ctx), Some(Page::Welcome));
    assert_eq!(flow(&mut app, &ctx), Some(Flow::First));
}

#[test]
fn a_flow_that_needs_a_power_cycle_goes_to_its_page_then_to_the_app() {
    use crate::model::Connection;
    use crate::views::{install, state, Cx};
    use oc_device::install::Flow;
    let (mut app, ctx) = with_connection(Some(Connection::Ready));
    {
        let mut cx = Cx { device: app.device.as_mut(), ui: &mut app.ui };
        install::demo_shot(&ctx, &mut cx, Flow::First, install::Stage::Done, true);
    }
    state::with(&ctx, |s| {
        if let Some(d) = s.install.as_mut().and_then(|x| x.demo.as_mut()) {
            d.progress.power_cycle = true;
        }
    });
    let visible = |app: &mut App| {
        let mut cx = Cx { device: app.device.as_mut(), ui: &mut app.ui };
        install::visible(&ctx, &mut cx)
    };
    assert!(visible(&mut app));
    assert_eq!(install::page(&ctx), Some(install::Page::PowerCycle { unplugged: false }), "no Done screen");
    let mut sim = SimDevice::new();
    sim.connection = Some(Connection::Absent);
    app.device = Box::new(sim);
    assert!(visible(&mut app));
    assert_eq!(install::page(&ctx), Some(install::Page::PowerCycle { unplugged: true }));
    let mut sim = SimDevice::new();
    sim.connection = Some(Connection::Ready);
    app.device = Box::new(sim);
    assert!(!visible(&mut app), "OpenCompanion is back: the app");
}

#[test]
fn a_speaker_that_comes_back_as_opencompanion_opens_the_app_without_a_restart() {
    use crate::model::Connection;
    use crate::views::{install, Cx};
    let (mut app, ctx) = with_connection(Some(Connection::Stock));
    let visible = |app: &mut App| {
        let mut cx = Cx { device: app.device.as_mut(), ui: &mut app.ui };
        install::visible(&ctx, &mut cx)
    };
    let swap = |app: &mut App, c: Connection| {
        let mut sim = SimDevice::new();
        sim.connection = Some(c);
        app.device = Box::new(sim);
    };
    assert!(visible(&mut app));
    assert_eq!(install::page(&ctx), Some(install::Page::Welcome));
    swap(&mut app, Connection::Absent);
    assert!(visible(&mut app), "unplugged: the welcome stays");
    swap(&mut app, Connection::Ready);
    assert!(!visible(&mut app), "back as OpenCompanion: the app");
    assert_eq!(install::page(&ctx), None);
    // Also from the installer's first page, and from a power cycle that was never seen unplugged.
    for page in [install::Page::Installer, install::Page::PowerCycle { unplugged: false }] {
        let (mut app, ctx) = with_connection(Some(Connection::Stock));
        let mut cx = Cx { device: app.device.as_mut(), ui: &mut app.ui };
        install::page_shot(&ctx, &mut cx, oc_device::install::Flow::First, page);
        let mut sim = SimDevice::new();
        sim.connection = Some(Connection::Ready);
        app.device = Box::new(sim);
        let mut cx = Cx { device: app.device.as_mut(), ui: &mut app.ui };
        assert!(!install::visible(&ctx, &mut cx), "{page:?}");
    }
}

#[test]
fn a_cancelled_install_returns_to_settings() {
    use crate::app::state::{Pane, Screen};
    use crate::views::{install, state, Cx};
    use oc_device::install::{Chip, Flow};
    let (mut app, ctx) = app();
    {
        let mut cx = Cx { device: app.device.as_mut(), ui: &mut app.ui };
        install::demo_shot(&ctx, &mut cx, Flow::File(Chip::Dsp, std::path::PathBuf::from("x.out")), install::Stage::Confirm, false);
    }
    state::with(&ctx, |s| {
        if let Some(d) = s.install.as_mut().and_then(|x| x.demo.as_mut()) {
            d.progress.cancelled = true;
        }
    });
    {
        let mut cx = Cx { device: app.device.as_mut(), ui: &mut app.ui };
        assert!(install::settle_cancel(&ctx, &mut cx));
    }
    assert_eq!(install::page(&ctx), None, "the installer is closed");
    for a in std::mem::take(&mut app.ui.pending) {
        app.apply(&ctx, a, None);
    }
    assert_eq!(app.ui.screen, Screen::Settings);
    assert_eq!(app.ui.settings.pane, Pane::Firmware);
}

#[test]
fn an_edit_over_the_boost_limit_stops_at_it() {
    use crate::model::{boost, Band};
    let (mut app, ctx) = app();
    run(&mut app, &ctx, Action::ActivateProfile(1));
    app.ui.eq.draft.get_mut(Target::Global).push(Band::new(BandType::Peak, 1000.0, 12.0, 4.0));
    run(&mut app, &ctx, Action::Edited(Change::Struct));
    assert!(!app.ui.eq.limited);
    // A second +12 dB bell on the same frequency (a drag): stopped at +15 dB.
    app.ui.eq.draft.get_mut(Target::Global).push(Band::new(BandType::Peak, 1000.0, 12.0, 4.0));
    run(&mut app, &ctx, Action::Edited(Change::Value));
    assert!(app.ui.eq.limited);
    assert!(boost::peak(app.ui.eq.draft.get(Target::Global)) <= boost::LIMIT + 0.05);
    assert_eq!(app.ui.eq.draft.get(Target::Global)[0].gain, 12.0, "the first band is kept");
}
