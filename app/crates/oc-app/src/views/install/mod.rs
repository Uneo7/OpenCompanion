pub mod backup;
pub mod demo;
pub mod explain;
mod pace;
mod parts;
mod power_cycle;
mod welcome;

use eframe::egui::{self, Context, Id, Pos2, Rect, Vec2};
use oc_device::install::{Chip, ChipVersion, Flow, Phase, Plan, Progress};

use super::{state, Cx};
use crate::app::actions::Action;
use crate::app::state::{Pane, Screen};
use crate::i18n::tr;
use crate::model::format::{from_label, version_label};
use crate::theme::motion;
use crate::widgets::dialog::{Body, PlanLine};

#[derive(Clone)]
pub struct Session {
    pub demo: Option<demo::Demo>,
    /// Onboarding: the speaker runs the Bose firmware (or a mix): the installer is the whole app until it is done.
    pub onboarding: bool,
    pub page: Page,
}

/// What the session shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    /// The installer (confirm, progress, result).
    #[default]
    Installer,
    /// The welcome page that invites to install (the speaker runs the original Bose firmware).
    Welcome,
    /// After a completed restore: unplug the speaker from the mains; `unplugged` once it has left.
    PowerCycle { unplugged: bool },
}

/// The page shown, if any.
pub fn page(ctx: &Context) -> Option<Page> {
    state::with(ctx, |s| s.install.as_ref().map(|x| x.page))
}

fn set_page(ctx: &Context, page: Page) {
    state::with(ctx, |s| {
        if let Some(x) = s.install.as_mut() {
            x.page = page;
        }
    });
}

/// The first page of the onboarding for `flow`: the welcome for a first install, else the installer (a resume).
fn first_page(flow: &Flow) -> Page {
    if *flow == Flow::First { Page::Welcome } else { Page::Installer }
}

/// The flow a speaker needs before the app can be used: the first install on the Bose firmware, the update
/// (as a resume) when one chip already runs ours. `None`: the app opens normally (ready, absent, recovery…).
pub fn onboarding_flow(c: &crate::model::Connection, restore_pending: bool) -> Option<Flow> {
    use crate::model::Connection;
    match c {
        // An unfinished restore to the Bose firmware is finished first (reinstalling stays a second choice).
        Connection::Stock | Connection::DspNotReady(_) if restore_pending => Some(Flow::Restore),
        Connection::Stock => Some(Flow::First),
        Connection::DspNotReady(_) => Some(Flow::Update),
        _ => None,
    }
}

pub fn onboarding(ctx: &Context) -> bool {
    state::with(ctx, |s| s.install.as_ref().is_some_and(|x| x.onboarding))
}

fn hw_progress(cx: &mut Cx) -> Option<Progress> {
    cx.device.installer().map(|i| i.progress())
}

pub fn progress(ctx: &Context, cx: &mut Cx) -> Option<Progress> {
    match state::with(ctx, |s| s.install.as_ref().map(|x| x.demo.as_ref().map(|d| d.progress.clone()))) {
        Some(Some(p)) => Some(p),
        Some(None) => hw_progress(cx),
        None => None,
    }
}

fn busy(phase: Phase) -> bool {
    matches!(phase, Phase::Running | Phase::AwaitingConfirmation)
}

/// Whether the installer view is shown. A hardware flow started elsewhere (the offer at connection) opens it,
/// and so does a failure, unless the user already left the view on that failure.
pub fn visible(ctx: &Context, cx: &mut Cx) -> bool {
    // The app never works on a speaker that does not run OpenCompanion: the Bose firmware (or a mix) only ever
    // gets the onboarding, at launch, on a replug, after a restore and after a failure.
    let conn = cx.device.connection();
    let needed = onboarding_flow(&conn, cx.device.restore_pending());
    // The power cycle page follows the speaker: it leaves the bus, then comes back (as the Bose firmware: the
    // welcome takes over below).
    if let Some(Page::PowerCycle { unplugged }) = page(ctx) {
        let gone = conn == crate::model::Connection::Absent;
        if gone && !unplugged {
            set_page(ctx, Page::PowerCycle { unplugged: true });
        } else if unplugged && !gone {
            state::with(ctx, |s| s.install = None);
        }
    }
    // OpenCompanion detected (a replug, the end of a power cycle): an onboarding that is not running a flow is
    // over, the app opens. A flow's own result (Done, Failed) stays until the user leaves it.
    if conn == crate::model::Connection::Ready {
        let idle = progress(ctx, cx).is_none_or(|p| p.phase == Phase::Confirm);
        let over = state::with(ctx, |s| {
            s.install.as_ref().is_some_and(|x| {
                x.onboarding && (matches!(x.page, Page::Welcome | Page::PowerCycle { .. }) || (x.page == Page::Installer && idle))
            })
        });
        if over {
            state::with(ctx, |s| s.install = None);
        }
    }
    // A completed flow that needs a mains power cycle (the new USB controller firmware starts then): straight to
    // the power cycle page; the app (or the welcome) follows once the speaker is back.
    if page(ctx) == Some(Page::Installer) && progress(ctx, cx).is_some_and(|p| p.phase == Phase::Done && p.power_cycle) {
        set_page(ctx, Page::PowerCycle { unplugged: false });
    }
    if let Some(onboarding) = state::with(ctx, |s| s.install.as_ref().map(|x| x.onboarding)) {
        // A flow already on screen keeps its own screens (a restore's Done included) until Finish; on such a
        // speaker they are part of the onboarding: no tabs, no way out.
        if needed.is_some() && !onboarding {
            state::with(ctx, |s| {
                if let Some(x) = s.install.as_mut() {
                    x.onboarding = true;
                }
            });
        }
        return true;
    }
    if let Some(flow) = needed {
        if cx.device.installer().is_none() {
            let page = first_page(&flow);
            let demo = demo::Demo::new(flow, &cx.device.status());
            state::with(ctx, |s| s.install = Some(Session { demo: Some(demo), onboarding: true, page }));
            return true;
        }
        let mut page = first_page(&flow);
        if let Some(i) = cx.device.installer() {
            match i.phase() {
                // Running or failed: that flow, as the onboarding.
                Phase::Running | Phase::AwaitingConfirmation | Phase::Failed => page = Page::Installer,
                // A finished restore leaves the Bose firmware: the welcome (first install) comes next, even
                // while the restore marker is still being re-read.
                Phase::Done if i.progress().flow == Flow::Restore => {
                    i.open(Flow::First);
                    page = Page::Welcome;
                }
                // Always opened: the versions (current and target) are known before the start.
                _ => {
                    i.open(flow);
                }
            }
        }
        state::with(ctx, |s| s.install = Some(Session { demo: None, onboarding: true, page }));
        return true;
    }
    let Some(phase) = cx.device.installer().map(|i| i.phase()) else { return false };
    let conn = cx.device.connection();
    state::with(ctx, |s| {
        if busy(phase) {
            s.install_dismissed = false;
        }
        let open = busy(phase) || (phase == Phase::Failed && !s.install_dismissed);
        if open {
            s.install = Some(Session { demo: None, onboarding: onboarding_flow(&conn, false).is_some(), page: Page::Installer });
        }
        open
    })
}

/// The write plan may be shown: the bar has reached 100 % on the step that built it.
pub fn plan_ready(ctx: &Context, p: &Progress) -> bool {
    pace::plan_ready(ctx, p)
}

/// The write plan as the user sees it: the flow's title, one line per chip concerned (current → target) and one
/// short warning. The technical plan (areas, addresses, pages) stays in the install log only.
pub fn plan_dialog(p: &Progress, plan: &Plan) -> (String, Body) {
    // Which chip the plan writes: read from its technical lines, never shown.
    let tech = format!("{} {}", plan.title, plan.writes.join(" "));
    let concerned = |c: Chip| match c {
        Chip::Dsp => tech.contains("U7001") || tech.contains("DSP"),
        Chip::Usb => tech.contains("U6002") || tech.contains("TAS"),
    };
    let mut chips: Vec<&ChipVersion> = p.versions.iter().filter(|v| concerned(v.chip)).collect();
    if chips.is_empty() {
        chips = p.versions.iter().collect();
    }
    let lines = chips
        .iter()
        .map(|v| PlanLine {
            name: format!("{} · {} → {}", tr(v.chip.key()), from_label(&v.from, v.from_stock, v.from_probable), version_label(&v.to, v.to_stock)),
            detail: String::new(),
        })
        .collect();
    let mut warning = tr("inst.plan.warn").to_owned();
    if chips.iter().any(|v| v.chip == Chip::Dsp) {
        warning = format!("{warning} {}", tr("inst.plan.sound"));
    }
    // A version older than the one installed: allowed, said explicitly.
    let downgrade = chips.iter().any(|v| matches!((&v.from, &v.to), (Some(f), Some(t)) if oc_device::install::ocfw::semver_lt(t, f)));
    if downgrade {
        warning = format!("{} {warning}", tr("inst.plan.downgrade"));
    }
    (tr(p.flow.title_key()).to_owned(), Body::Plan { sub: String::new(), lines, warning: Some(warning) })
}

pub fn running(ctx: &Context, cx: &mut Cx) -> bool {
    progress(ctx, cx).is_some_and(|p| busy(p.phase))
}

pub fn open(ctx: &Context, cx: &mut Cx, flow: Flow) {
    let status = cx.device.status();
    let demo = match cx.device.installer() {
        Some(inst) => {
            if !inst.open(flow) {
                return;
            }
            None
        }
        None => Some(demo::Demo::new(flow, &status)),
    };
    cx.ui.menu = None;
    state::with(ctx, |s| {
        s.install = Some(Session { demo, onboarding: false, page: Page::Installer });
        s.install_dismissed = false;
    });
}

/// Leaves the view (a tab was clicked), unless a flow is writing. Leaving an interrupted flow keeps it closed
/// (it can be resumed from Settings › Firmware, which reopens it).
pub fn leave(ctx: &Context, cx: &mut Cx) {
    let writing = progress(ctx, cx).is_some_and(|p| busy(p.phase));
    if !writing && !onboarding(ctx) {
        close(ctx);
    }
}

fn close(ctx: &Context) {
    state::with(ctx, |s| {
        s.install = None;
        s.install_dismissed = true;
    });
}

pub(crate) enum Event {
    Start,
    /// Onboarding of an unfinished restore: reinstall OpenCompanion instead.
    Reinstall,
    Resume,
    Guide,
    /// Copies the version and the end of the log (an unsupported Bose firmware: to report it).
    CopyReport,
    Finish,
    /// Back to the welcome page (first install, nothing written yet).
    Back,
    /// The backup of the Bose firmware failed or is missing: try it again.
    RetryBackup,
}

pub fn show(ui: &mut egui::Ui, cx: &mut Cx, content: Rect) {
    let ctx = ui.ctx().clone();
    match page(&ctx) {
        Some(Page::Welcome) => {
            if welcome::show(ui, content) {
                set_page(&ctx, Page::Installer);
            }
            return;
        }
        Some(Page::PowerCycle { unplugged }) => {
            // "C'est fait": the welcome comes next (the speaker runs the Bose firmware again).
            if power_cycle::show(ui, content, unplugged) {
                state::with(&ctx, |s| s.install = None);
            }
            return;
        }
        _ => {}
    }
    let now = ctx.input(|i| i.time);
    let ticking = state::with(&ctx, |s| s.install.as_mut().and_then(|x| x.demo.as_mut()).is_some_and(|d| d.tick(now)));
    if ticking {
        ctx.request_repaint_after(std::time::Duration::from_millis(120));
    }
    let Some(p) = progress(&ctx, cx) else {
        state::with(&ctx, |s| s.install = None);
        return;
    };
    if settle_cancel(&ctx, cx) {
        return;
    }
    let p = pace::paced(&ctx, &p);
    let key = p.phase as u64 + 10 * u64::from(p.error.is_some());
    let k = motion::SPRING.at(motion::since(&ctx, Id::new("inst-phase"), key, 0.22));
    let area = Rect::from_min_size(Pos2::new(content.left(), content.top()), Vec2::new(content.width(), content.height()));
    let why = why(cx, &p);
    if let Some(ev) = parts::show(ui, &p, area, k, why, onboarding(&ctx)) {
        handle(&ctx, cx, ev, now);
    }
}

/// A page of the onboarding, as a screenshot shows it (`--shots`).
pub fn page_shot(ctx: &Context, cx: &mut Cx, flow: Flow, page: Page) {
    demo_shot(ctx, cx, flow, Stage::Confirm, true);
    set_page(ctx, page);
}

/// The user declined the write plan (nothing was written): back to where they came from, Settings › Firmware
/// (or the welcome during the onboarding), with a short toast. True when it did.
pub(crate) fn settle_cancel(ctx: &Context, cx: &mut Cx) -> bool {
    let Some(p) = progress(ctx, cx) else { return false };
    if !(p.phase == Phase::Confirm && p.cancelled) {
        return false;
    }
    let onboarding = onboarding(ctx);
    close(ctx);
    // Opened again: the flag is cleared and the next visit starts clean.
    if let Some(i) = cx.device.installer() {
        i.open(p.flow.clone());
    }
    cx.ui.toast.show(ctx, tr("inst.cancelled.install"));
    if !onboarding {
        cx.act(Action::Show(Screen::Settings));
        cx.act(Action::ShowPane(Pane::Firmware));
    }
    true
}

/// What to send for an unsupported Bose firmware: the versions, the error and the end of the log.
pub(crate) fn report(p: &Progress, status: &crate::model::device::Status) -> String {
    let mut out = format!(
        "OpenCompanion {}\nDSP: {}\nUSB: {}\nflow: {:?}\nerror: {}\n\n",
        env!("CARGO_PKG_VERSION"),
        status.firmware_dsp,
        status.firmware_usb,
        p.flow,
        p.error.as_deref().unwrap_or("-")
    );
    let tail = p.log.len().saturating_sub(40);
    for l in &p.log[tail..] {
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// Applies a button of the installer view (scenario tests).
#[cfg(test)]
pub(crate) fn press(ctx: &Context, cx: &mut Cx, ev: Event) {
    handle(ctx, cx, ev, 0.0);
}

/// Why the flow cannot start or stopped ([`explain`]); the restore also refuses a speaker already stock.
fn why(cx: &mut Cx, p: &Progress) -> parts::Why {
    let release = cx.device.installer().and_then(|i| i.release_status());
    let state = backup::backup_state(cx);
    let asks = p.phase == Phase::Confirm || (p.phase == Phase::Failed && !p.can_resume);
    let stock = cx.device.connection() == crate::model::Connection::Stock;
    let blocked = if asks && p.flow == Flow::Restore {
        explain::restore_blocker(&state, stock, cx.device.restore_pending(), release.as_ref())
    } else {
        None
    };
    let failure = p.error.as_deref().and_then(|e| explain::run_error(e, release.as_ref()));
    let retry_backup = blocked.is_some() && matches!(state, oc_device::install::BackupState::Missing | oc_device::install::BackupState::Failed(_));
    parts::Why { blocked, failure, retry_backup }
}

fn handle(ctx: &Context, cx: &mut Cx, ev: Event, now: f64) {
    let demo = state::with(ctx, |s| s.install.as_ref().is_some_and(|x| x.demo.is_some()));
    match ev {
        Event::Start | Event::Resume if demo => state::with(ctx, |s| {
            if let Some(d) = s.install.as_mut().and_then(|x| x.demo.as_mut()) {
                if matches!(ev, Event::Resume) {
                    d.resume(now);
                } else {
                    d.start(now);
                }
            }
        }),
        Event::Start | Event::Resume => {
            if let Some(i) = cx.device.installer() {
                i.start();
            }
        }
        Event::Reinstall => {
            if let Some(i) = cx.device.installer() {
                i.open(Flow::Update);
            }
        }
        Event::Guide => cx.act(Action::View(super::actions::ViewAction::RecoveryGuide)),
        Event::CopyReport => {
            if let Some(p) = progress(ctx, cx) {
                ctx.copy_text(report(&p, &cx.device.status()));
                cx.ui.toast.show(ctx, tr("inst.err.copied"));
            }
        }
        Event::Back => set_page(ctx, Page::Welcome),
        Event::RetryBackup => {
            backup::request(cx);
        }
        // After the onboarding the app opens on Home; after an install from Settings, back to Firmware.
        // A completed restore ends with the mains power cycle (its own page), then the welcome.
        Event::Finish if progress(ctx, cx).is_some_and(|p| p.flow == Flow::Restore && p.phase == Phase::Done) => {
            set_page(ctx, Page::PowerCycle { unplugged: false });
        }
        Event::Finish if onboarding(ctx) => {
            close(ctx);
            cx.act(Action::Show(Screen::Home));
        }
        Event::Finish => {
            close(ctx);
            cx.act(Action::Show(Screen::Settings));
            cx.act(Action::ShowPane(Pane::Firmware));
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Stage {
    Confirm,
    Run(usize, f32),
    Fail(usize, f32),
    Done,
}

pub fn demo_shot(ctx: &Context, cx: &mut Cx, flow: Flow, stage: Stage, onboarding: bool) {
    let mut d = demo::Demo::new(flow, &cx.device.status());
    match stage {
        Stage::Confirm => {}
        Stage::Run(i, pct) => d.jump(i, pct),
        Stage::Fail(i, pct) => {
            d.jump(i, pct);
            d.fail_now();
        }
        Stage::Done => d.finish(),
    }
    // Frozen: the timer never ticks in a screenshot.
    d.frozen = true;
    state::with(ctx, |s| s.install = Some(Session { demo: Some(d), onboarding, page: Page::Installer }));
}
