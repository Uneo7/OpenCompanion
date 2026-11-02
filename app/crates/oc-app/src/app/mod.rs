pub mod actions;
mod corners;
mod dispatch;
pub mod files;
mod hardware;
mod overlays;
pub mod shots;
mod shots_board;
pub mod state;
#[cfg(test)]
mod tests;
pub mod window;

use eframe::egui::{self, CentralPanel, Frame, Id, Pos2, Rect, Stroke, StrokeKind, Vec2};

use crate::cli::Options;
use crate::config::{self, AppConfig, ThemePref};
use crate::model::{Device, HwDevice, SimDevice};
use crate::theme::{self, metrics, motion, typography, Theme};
use crate::views::{self, Cx};
use crate::widgets::background;

use state::{Screen, UiState};

pub struct App {
    pub(crate) device: Box<dyn Device>,
    pub(crate) ui: UiState,
    shots: Option<shots::Runner>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, opts: Options) -> Self {
        corners::round(cc);
        let ctx = &cc.egui_ctx;
        typography::install(ctx);
        motion::set_reduced(ctx, opts.reduced_motion || opts.shots.is_some());
        // The window has one fixed size: no zooming with Ctrl +/−.
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        ctx.style_mut(|s| {
            s.interaction.selectable_labels = false;
            s.visuals.text_cursor.blink = true;
        });
        // Files of the earlier storage layout move to the current one (logs, Linux folder names), once.
        if opts.shots.is_none() {
            match oc_device::paths::migrate() {
                Ok(moved) if !moved.is_empty() => oc_device::install::log_file::append(&moved),
                Ok(_) => {}
                Err(e) => oc_device::install::log_file::append(&[format!("storage migration: {e}")]),
            }
        }
        let saved = AppConfig::load();
        match opts.lang {
            Some(l) => {
                let _ = config::set_language(l); // a read-only config dir only loses the persistence
            }
            None => saved.apply(),
        }
        if let Some(dir) = &opts.firmware {
            let _ = AppConfig { firmware_dir: Some(dir.clone()), ..AppConfig::load() }.save();
        }
        let device: Box<dyn Device> = if opts.sim || opts.shots.is_some() {
            Box::new(SimDevice::new())
        } else {
            let sources = hardware::sources(opts.firmware.as_deref().or(saved.firmware_dir.as_deref()));
            let repaint = ctx.clone();
            Box::new(HwDevice::new(sources, move || repaint.request_repaint()))
        };
        // Appearance: the saved preference; `--light` forces Clair for this run; screenshots never use (nor
        // change) the saved one.
        let theme = match (opts.light, opts.shots.is_some()) {
            (true, _) => ThemePref::Light,
            (false, true) => ThemePref::Dark,
            (false, false) => saved.theme,
        };
        views::prefs::init(ctx, theme, opts.shots.is_none());
        let mut ui = UiState::new(device.as_ref());
        ui.screen = opts.screen;
        ui.light = opts.light;
        Self { device, ui, shots: opts.shots.map(shots::Runner::new) }
    }

    pub(crate) fn reset(&mut self) {
        self.device = Box::new(SimDevice::new());
        self.ui = UiState::new(self.device.as_ref());
    }

    /// Resolves this frame's colours: theme cross-fade (.4 s) and the active profile's gradient (.7 s).
    fn publish_theme(&self, ctx: &egui::Context) -> std::sync::Arc<Theme> {
        let light_t = motion::tween(ctx, Id::new("theme-light"), if self.ui.light { 1.0 } else { 0.0 }, 0.4, motion::EASE);
        let hue = self.device.profiles()[self.device.active_profile()].hue();
        let a1 = motion::color(ctx, Id::new("theme-a1"), theme::rgb(hue.a), 0.7, motion::EASE);
        let a2 = motion::color(ctx, Id::new("theme-a2"), theme::rgb(hue.b), 0.7, motion::EASE);
        theme::set(ctx, Theme::resolve(light_t, a1, a2));
        theme::get(ctx)
    }

    /// Centre of the clear zone of the background lines: the photo on Home, the middle elsewhere (.3 s).
    fn fade_center(&self, ctx: &egui::Context, main: Rect) -> Vec2 {
        let target = match self.ui.screen {
            Screen::Home => views::home::fade_center(main),
            _ => background::FADE_DEFAULT,
        };
        Vec2::new(
            motion::tween(ctx, Id::new("fade-x"), target.x, 0.3, motion::CUBIC_OUT),
            motion::tween(ctx, Id::new("fade-y"), target.y, 0.3, motion::CUBIC_OUT),
        )
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        // A close request from the OS (Alt+F4, Cmd+Q, the taskbar) asks first, like the window's close button.
        if ctx.input(|i| i.viewport().close_requested()) && !self.ui.close_confirmed {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.ui.dialog.is_none() {
                self.ui.act(actions::Action::AskClose);
            }
        }
        self.device.tick(now);
        // Telemetry and timers advance without input.
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
        self.watch_device(ctx);
        let t = self.publish_theme(ctx);

        let mut window = ctx.screen_rect();
        CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
            window = ui.max_rect();
            let radius = metrics::window_radius();
            background::paint_window(ui.painter(), window, radius, &t);
            let bar = Rect::from_min_size(window.min, Vec2::new(window.width(), metrics::TOOLBAR_H));
            let main = Rect::from_min_max(Pos2::new(window.left(), bar.bottom()), window.max);
            let fade = self.fade_center(ctx, main);
            background::paint_lines(ui.painter(), main, &t, fade);
            ui.painter().rect_stroke(window, radius, Stroke::new(1.0_f32, t.outline), StrokeKind::Inside);

            let mut cx = Cx { device: self.device.as_mut(), ui: &mut self.ui };
            views::chrome::title_bar(ui, &mut cx, bar);
            views::show(ui, &mut cx, main);
        });

        self.overlays(ctx, window);
        for a in std::mem::take(&mut self.ui.pending) {
            self.apply(ctx, a, None);
        }
        // Native file dialogs are modal: opened last, with the window as their parent.
        if let Some(pick) = self.ui.pick.take() {
            self.pick(ctx, frame, pick);
        }
        if let Some(mut runner) = self.shots.take() {
            let done = runner.step(ctx, self);
            if !done {
                self.shots = Some(runner);
            }
        }
    }
}
