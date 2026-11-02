use eframe::egui::{self, Align, Color32, Id, Pos2, Rect, Stroke, StrokeKind, Vec2};
use oc_device::install::{Chip, ChipVersion, Phase, Progress, StepState};

use super::Event;
use crate::assets::icons::{self, Icon};
use crate::i18n::{t_with, tr};
use crate::model::format::{from_label, version_label};
use crate::theme::{self, typography::W, Theme};
use crate::widgets::{button, text, text::st};

const GAP: f32 = 22.0;
const BIG_H: f32 = 150.0;
const LINE_H: f32 = 30.0;
const BAR_H: f32 = 14.0;
const LBL_H: f32 = 16.0;
const ROW_H: f32 = 36.0;

/// Why the flow cannot start or stopped, in the user's words ([`super::explain`]).
#[derive(Default)]
pub(crate) struct Why {
    /// The start is refused (the restore without a verified backup, or on a speaker already stock).
    pub blocked: Option<String>,
    /// What stopped the flow, when known (else a lost connection).
    pub failure: Option<String>,
    /// The backup can be made again from here.
    pub retry_backup: bool,
}

struct View<'a> {
    p: &'a Progress,
    blocked: bool,
    why: Why,
    error: bool,
    critical: bool,
    /// The onboarding's first screen also says what will happen (backup, then install).
    onboarding: bool,
}

pub(crate) fn show(ui: &mut egui::Ui, p: &Progress, area: Rect, k: f32, why: Why, onboarding: bool) -> Option<Event> {
    let ctx = ui.ctx().clone();
    let t = theme::get(&ctx);
    let v = View { p, blocked: why.blocked.is_some(), why, error: p.phase == Phase::Failed, critical: p.critical(), onboarding };
    let mut ev = None;

    let title = if onboarding && p.phase == Phase::Confirm { tr("onboard.title") } else { tr(p.flow.title_key()) };
    text::line(ui.painter(), title, &st::page(), t.text, area.left(), area.top() + 4.0, 32.0, Align::Min);
    if p.phase == Phase::Done {
        let b = button::Button::new(tr("inst.finish")).kind(button::Kind::Primary);
        let w = b.size(&ctx).x;
        if b.show(ui, Pos2::new(area.right() - w, area.top() + 2.0), Id::new("inst-close")).clicked() {
            ev = Some(Event::Finish);
        }
    }

    // `.inst`: centred in what is left under the head (24 px gap, 40 px bottom padding).
    let line = has_line(&v);
    let rows = rows_height(&v);
    let mut h = BIG_H + GAP + BAR_H + (GAP - 14.0) + LBL_H;
    if line {
        h += GAP + LINE_H;
    }
    if rows > 0.0 {
        h += GAP + rows;
    }
    let body = Rect::from_min_max(Pos2::new(area.left(), area.top() + 40.0 + 24.0), Pos2::new(area.right(), area.bottom() - 40.0));
    let mut y = body.top() + ((body.height() - h) / 2.0).max(0.0) + 6.0 * (1.0 - k);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area.expand(8.0)));
    child.set_opacity(k);
    let ui = &mut child;

    big(ui, &v, area.left() - 8.0, y, &t);
    if !v.error {
        versions(ui, p, area.right(), y + BIG_H / 2.0, &t);
    }
    y += BIG_H + GAP;
    if line {
        status_line(ui, &v, area.left(), y, &t);
        y += LINE_H + GAP;
    }
    bar(ui, p, Rect::from_min_size(Pos2::new(area.left(), y), Vec2::new(area.width(), BAR_H)), &t);
    y += BAR_H + GAP - 14.0;
    labels(ui, p, Rect::from_min_size(Pos2::new(area.left(), y), Vec2::new(area.width(), LBL_H)), &t);
    y += LBL_H + GAP;
    if rows > 0.0 {
        if let Some(e) = action_rows(ui, &v, area.left(), y, &t) {
            ev = Some(e);
        }
    }
    ev
}

fn has_line(v: &View) -> bool {
    v.p.phase != Phase::Confirm || v.onboarding
}

fn rows_height(v: &View) -> f32 {
    match v.p.phase {
        Phase::Confirm => 20.0 + 14.0 + ROW_H,
        Phase::Failed => ROW_H,
        Phase::Running | Phase::AwaitingConfirmation if v.p.writing() => 20.0,
        _ => 0.0,
    }
}

fn big(ui: &egui::Ui, v: &View, x: f32, top: f32, t: &Theme) {
    let ctx = ui.ctx();
    let (num, unit) = match v.p.phase {
        Phase::Confirm => (v.p.flow.minutes().to_string(), tr("inst.unit.min")),
        _ => (v.p.percent().to_string(), "%"),
    };
    let num_style = st::inter_(168.0, W::SemiBold).tracking(-0.06);
    let unit_style = st::inter_(60.0, W::SemiBold).tracking(-0.03);
    let (num_c, unit_c) = if v.error {
        (t.crit, t.crit)
    } else if v.critical {
        (t.text, t.danger_fill)
    } else {
        (t.text, t.a1)
    };
    let r = text::line(ui.painter(), &num, &num_style, num_c, x, top, BIG_H, Align::Min);
    let base = top + text::baseline_in_box(ctx, &num_style, BIG_H);
    let ut = base - text::baseline_in_box(ctx, &unit_style, 60.0);
    text::line(ui.painter(), unit, &unit_style, unit_c, r.right() + 6.0, ut, 60.0, Align::Min);
}

fn group_key(p: &Progress, key: &'static str) -> &'static str {
    if key.starts_with("inst.chip.") {
        key
    } else if p.steps.iter().any(|s| s.id == oc_device::install::StepId::Backup) {
        // First install: the backup and the preparation share one group.
        "inst.group.backup_prep"
    } else {
        p.flow.prep_key()
    }
}

fn chip_icon(c: Chip) -> Icon {
    match c {
        Chip::Dsp => icons::CHIP,
        Chip::Usb => icons::USB,
    }
}

fn versions(ui: &egui::Ui, p: &Progress, right: f32, cy: f32, t: &Theme) {
    let row_h = 16.0 + 3.0 + 20.0;
    let h = p.versions.len() as f32 * row_h + p.versions.len().saturating_sub(1) as f32 * 14.0;
    let left = right - 260.0;
    let mut y = cy - h / 2.0;
    for cv in &p.versions {
        chip_icon(cv.chip).paint(ui.painter(), Pos2::new(left + 7.0, y + 8.0), 14.0, t.a1);
        text::line(ui.painter(), tr(cv.chip.key()), &st::inter_(12.0, W::Regular), t.muted, left + 21.0, y, 16.0, Align::Min);
        let vy = y + 19.0;
        let target = |x: f32, cv: &ChipVersion| {
            let style = if cv.to.is_some() { st::mono_(15.0, W::Medium) } else { st::inter_(15.0, W::SemiBold) };
            text::line(ui.painter(), &version_label(&cv.to, cv.to_stock), &style, t.text, x, vy, 20.0, Align::Min)
        };
        let same = cv.unchanged();
        if p.phase == Phase::Done || same {
            let r = target(left, cv);
            if same && p.phase != Phase::Done {
                text::line(ui.painter(), tr("inst.confirm.usb.update"), &st::inter_(12.0, W::Regular), t.muted, r.right() + 8.0, vy + 1.0, 20.0, Align::Min);
            }
        } else {
            let r = text::line(ui.painter(), &from_label(&cv.from, cv.from_stock, cv.from_probable), &st::inter_(15.0, W::Regular), t.muted, left, vy, 20.0, Align::Min);
            let a = text::line(ui.painter(), "→", &st::inter_(15.0, W::Regular), t.a1, r.right() + 8.0, vy, 20.0, Align::Min);
            target(a.right() + 8.0, cv);
        }
        y += row_h + 14.0;
    }
}

fn status_line(ui: &egui::Ui, v: &View, x: f32, top: f32, t: &Theme) {
    let main = st::inter_(22.0, W::SemiBold).tracking(-0.01);
    let sub = st::inter_(15.0, W::Regular);
    let (icon, title, rest): (Option<(Icon, Color32)>, String, String) = match v.p.phase {
        Phase::Done => (Some((icons::CHECK, t.a1)), tr(v.p.flow.done_key()).into(), String::new()),
        Phase::Failed => {
            let chip = tr(group_key(v.p, v.p.error_chip_key()));
            let not_ours = v.p.error.as_deref().is_some_and(|e| e.contains(oc_device::install::identity::NOT_OURS));
            let why = if let Some(b) = &v.why.blocked {
                b.clone()
            } else if let Some(f) = &v.why.failure {
                f.clone()
            } else if not_ours {
                // A file refused by the identity checks: nothing was written.
                tr("inst.err.not_ours").to_owned()
            } else {
                t_with("inst.err.lost", &[("chip", &chip)])
            };
            (Some((icons::WARN, t.crit)), tr("inst.err.title").into(), why)
        }
        Phase::Confirm => (None, tr(match v.p.flow {
            oc_device::install::Flow::First => "onboard.steps.first",
            oc_device::install::Flow::Restore => "onboard.steps.restore",
            _ => "onboard.steps.resume",
        }).into(), String::new()),
        _ => {
            let step = v.p.current_key().map_or("", tr).to_owned();
            (None, step, String::new())
        }
    };
    let mut cx = x;
    if let Some((ic, c)) = icon {
        ic.paint(ui.painter(), Pos2::new(cx + 11.0, top + LINE_H / 2.0), 22.0, c);
        cx += 22.0 + 14.0;
    }
    let r = text::line(ui.painter(), &title, &main, t.text, cx, top, LINE_H, Align::Min);
    if !rest.is_empty() {
        let base = top + text::baseline_in_box(ui.ctx(), &main, LINE_H);
        let st_ = base - text::baseline_in_box(ui.ctx(), &sub, 20.0);
        // Within the content width: a long reason is shortened with an ellipsis.
        let max = ui.max_rect().right() - (r.right() + 14.0) - 8.0;
        let rest = text::elide(ui.ctx(), &rest, &sub, max.max(40.0));
        text::line(ui.painter(), &rest, &sub, t.muted, r.right() + 14.0, st_, 20.0, Align::Min);
    }
}

fn bar(ui: &egui::Ui, p: &Progress, rect: Rect, t: &Theme) {
    let segs: Vec<_> = p.steps.iter().enumerate().filter(|(_, s)| s.weight > 0).collect();
    let total: f32 = segs.iter().map(|(_, s)| s.weight as f32).sum();
    if total <= 0.0 {
        return;
    }
    let gaps = 4.0 * (segs.len() as f32 - 1.0);
    let mut x = rect.left();
    let inner = rect.shrink2(Vec2::new(0.0, 2.0));
    for (i, s) in segs {
        let w = (rect.width() - gaps) * s.weight as f32 / total;
        let r = Rect::from_min_size(Pos2::new(x, inner.top()), Vec2::new(w, inner.height()));
        let critical = s.id.critical();
        let f = match s.state {
            StepState::Done | StepState::Skipped => 1.0,
            StepState::Pending => 0.0,
            StepState::Running | StepState::Error => f32::from(s.percent) / 100.0,
        };
        let cur = p.current == Some(i) && p.phase != Phase::Done;
        let (track, fill) = if critical { (theme::alpha(t.danger_fill, 0.38), t.danger_fill) } else { (t.track, t.a1) };
        let fill = if s.state == StepState::Error { t.crit } else { fill };
        if cur && s.state == StepState::Running {
            let ring = if critical { theme::alpha(t.danger_fill, 0.5) } else { theme::alpha(t.a1, 0.45) };
            ui.painter().rect_stroke(r, 5.0, Stroke::new(2.0_f32, ring), StrokeKind::Outside);
        }
        ui.painter().rect_filled(r, 5.0, track);
        if f > 0.0 {
            let fr = Rect::from_min_size(r.min, Vec2::new((w * f).max(r.height().min(w)), r.height()));
            ui.painter().rect_filled(fr, 5.0, fill);
        }
        x += w + 4.0;
    }
}

fn labels(ui: &egui::Ui, p: &Progress, rect: Rect, t: &Theme) {
    let groups = p.groups();
    let total: f32 = groups.iter().map(|g| g.1 as f32).sum();
    if total <= 0.0 {
        return;
    }
    let gaps = 4.0 * (groups.len() as f32 - 1.0);
    let mut x = rect.left();
    let style = st::inter_(11.0, W::SemiBold);
    for (key, w) in groups {
        let width = (rect.width() - gaps) * w as f32 / total;
        let icon = match key {
            "inst.chip.dsp" => icons::CHIP,
            "inst.chip.usb" => icons::USB,
            _ => icons::DRIVE,
        };
        icon.paint(ui.painter(), Pos2::new(x + 6.5, rect.center().y), 13.0, t.a1);
        let painter = ui.painter().with_clip_rect(Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(width, rect.height())));
        text::line(&painter, tr(group_key(p, key)), &style, t.a1, x + 19.0, rect.top(), LBL_H, Align::Min);
        x += width + 4.0;
    }
}

fn note(ui: &egui::Ui, x: f32, top: f32, key: &str, kind: u8, t: &Theme) {
    note_text(ui, x, top, &crate::i18n::t(key), kind, t);
}

fn note_text(ui: &egui::Ui, x: f32, top: f32, line: &str, kind: u8, t: &Theme) {
    let (ic_c, txt_c, w) = match kind {
        0 => (t.warn, t.text, W::SemiBold),
        1 => (t.a1, t.muted, W::Regular),
        _ => (t.danger_fill, t.danger_fill, W::SemiBold),
    };
    icons::WARN.paint(ui.painter(), Pos2::new(x + 9.0, top + 10.0), 18.0, ic_c);
    text::line(ui.painter(), line, &st::inter_(14.0, w), txt_c, x + 26.0, top, 20.0, Align::Min);
}

fn action_rows(ui: &mut egui::Ui, v: &View, x: f32, top: f32, t: &Theme) -> Option<Event> {
    let ctx = ui.ctx().clone();
    match v.p.phase {
        Phase::Confirm => {
            if let Some(b) = &v.why.blocked {
                note_text(ui, x, top, b, 2, t);
            } else {
                note(ui, x, top, "inst.warn.unplug", 0, t);
            }
            // Onboarding of an unfinished restore: "Terminer la restauration" first, reinstalling as a second choice.
            let restore = v.onboarding && v.p.flow == oc_device::install::Flow::Restore;
            let label = if restore { tr("onboard.restore.finish") } else { tr(v.p.flow.start_key()) };
            let go = button::Button::new(label).icon(icons::PLAY).kind(button::Kind::Primary).enabled(!v.blocked);
            let y = top + 20.0 + 14.0;
            let gw = go.size(&ctx).x;
            let mut ev = go.show(ui, Pos2::new(x, y), Id::new("inst-go")).clicked().then_some(Event::Start);
            let mut next = x + gw + 14.0;
            if v.why.retry_backup {
                let retry = button::Button::new(tr("fw.backup.retry")).icon(icons::ROTATE).kind(button::Kind::Quiet);
                let rw = retry.size(&ctx).x;
                if retry.show(ui, Pos2::new(next, y), Id::new("inst-retry-backup")).clicked() {
                    ev = Some(Event::RetryBackup);
                }
                next += rw + 14.0;
            }
            if restore {
                let other = button::Button::new(tr("onboard.reinstall")).kind(button::Kind::Quiet);
                if other.show(ui, Pos2::new(next, y), Id::new("inst-reinstall")).clicked() {
                    ev = Some(Event::Reinstall);
                }
            } else if v.onboarding && v.p.flow == oc_device::install::Flow::First {
                // The way out of the first install's page, before anything is written: the welcome.
                let back = button::Button::new(tr("onboard.back")).icon(icons::BACK).kind(button::Kind::Quiet);
                if back.show(ui, Pos2::new(x + gw + 14.0, y), Id::new("inst-back")).clicked() {
                    ev = Some(Event::Back);
                }
            }
            ev
        }
        Phase::Failed => {
            let guide = button::Button::new(tr("inst.err.guide")).icon(icons::INFO).kind(button::Kind::Quiet);
            let gw = guide.size(&ctx).x;
            let mut ev = guide.show(ui, Pos2::new(x, top), Id::new("inst-guide")).clicked().then_some(Event::Guide);
            let mut bx = x + gw + 14.0;
            if v.p.can_resume {
                let resume = button::Button::new(tr("inst.err.resume")).icon(icons::ROTATE).kind(button::Kind::Primary);
                let rw = resume.size(&ctx).x;
                if resume.show(ui, Pos2::new(bx, top), Id::new("inst-resume")).clicked() {
                    ev = Some(Event::Resume);
                }
                bx += rw + 14.0;
            }
            // An unknown Bose version: the report to send, as an extra.
            if v.p.error.as_deref().and_then(super::explain::unsupported_stock).is_some() {
                let copy = button::Button::new(tr("inst.err.copy_report")).icon(icons::COPY).kind(button::Kind::Quiet);
                if copy.show(ui, Pos2::new(bx, top), Id::new("inst-copy")).clicked() {
                    ev = Some(Event::CopyReport);
                }
            }
            ev
        }
        _ => {
            if v.critical {
                note(ui, x, top, "inst.warn.critical", 2, t);
            } else {
                note(ui, x, top, "inst.warn", 1, t);
            }
            None
        }
    }
}
