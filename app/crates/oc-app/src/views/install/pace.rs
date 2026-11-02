//! What the bar shows, paced so that the installer never looks frozen:
//! - the running step fills on its own, time-based, easing towards 95 % (`1 − e^(−t/τ)`), unless the speaker
//!   reports more (pages written); long silent steps (the file check reading the EEPROM, the restart waiting for
//!   the speaker to come back) keep moving;
//! - every step's fill glides to its value (0.6 s minimum from 0 to 100), so a step that ends at once still
//!   visibly goes from 0 to 100 %;

use eframe::egui::{Context, Id};
use oc_device::install::{Phase, Progress, StepId, StepState};

use crate::theme::motion;

const GLIDE_S: f32 = 0.6;
/// A skipped step (already up to date) fills faster, and only after the step before it is full.
const SKIP_GLIDE_S: f32 = 0.2;

/// Time constant of the estimate (s): about 2.3 τ to reach 90 %.
fn tau(id: StepId) -> f32 {
    match id {
        // Reads the EEPROM and builds the plan: 10–15 s.
        StepId::FileCheck | StepId::RestoreCheck => 5.0,
        StepId::DspVerify | StepId::UsbVerify => 4.0,
        StepId::DspCritical => 0.4,
        _ => 6.0,
    }
}

#[derive(Clone, Copy)]
struct Entered {
    step: usize,
    at: f64,
}

pub fn paced(ctx: &Context, p: &Progress) -> Progress {
    let mut out = p.clone();
    let now = ctx.input(|i| i.time);
    let key = Id::new("inst-pace-entered");
    let entered = match p.current {
        Some(i) => {
            let e = ctx.data(|d| d.get_temp::<Entered>(key)).filter(|e| e.step == i).unwrap_or(Entered { step: i, at: now });
            ctx.data_mut(|d| d.insert_temp(key, e));
            Some(e)
        }
        None => None,
    };
    let reduced = motion::reduced(ctx);
    // Segments fill left to right: a step only moves once the one before it is shown full.
    let mut prev_full = true;
    for (i, s) in out.steps.iter_mut().enumerate() {
        let target = match s.state {
            StepState::Done | StepState::Skipped => 100.0,
            StepState::Pending => 0.0,
            StepState::Error => f32::from(s.percent),
            StepState::Running if p.phase == Phase::AwaitingConfirmation => 100.0,
            StepState::Running => {
                let t = entered.map_or(0.0, |e| (now - e.at) as f32);
                let est = 95.0 * (1.0 - (-t / tau(s.id)).exp());
                if p.phase == Phase::Running {
                    ctx.request_repaint();
                }
                f32::from(s.percent).max(est)
            }
        };
        let id = Id::new(("inst-pace-step", i));
        let shown = if reduced || p.phase == Phase::Done {
            target
        } else {
            let prev = ctx.data(|d| d.get_temp::<(f32, f64)>(id)).map_or(target.min(0.0), |(v, _)| v);
            let last = ctx.data(|d| d.get_temp::<(f32, f64)>(id)).map_or(now, |(_, t)| t);
            let dt = (now - last) as f32;
            let secs = if s.state == StepState::Skipped { SKIP_GLIDE_S } else { GLIDE_S };
            let max_step = if prev_full { 100.0 / secs * dt } else { 0.0 };
            let v = if target > prev { (prev + max_step).min(target) } else { target };
            if v < target {
                ctx.request_repaint();
            }
            v
        };
        ctx.data_mut(|d| d.insert_temp(id, (shown, now)));
        prev_full = shown >= 99.5;
        // A finished step still gliding is drawn as running at its glide value.
        if matches!(s.state, StepState::Done | StepState::Skipped) && shown < 99.5 {
            s.state = StepState::Running;
        }
        s.percent = shown.round().clamp(0.0, 100.0) as u8;
    }
    if p.phase == Phase::Confirm {
        ctx.data_mut(|d| {
            d.remove::<Entered>(key);
            for i in 0..p.steps.len() {
                d.remove::<(f32, f64)>(Id::new(("inst-pace-step", i)));
            }
        });
    }
    out
}

pub fn plan_ready(ctx: &Context, p: &Progress) -> bool {
    p.current.is_none_or(|i| ctx.data(|d| d.get_temp::<(f32, f64)>(Id::new(("inst-pace-step", i)))).is_none_or(|(v, _)| v >= 99.5))
}
