//! Windows and macOS: the tray icon (tray-icon) and its menu, Open OpenCompanion and Quit. Its events arrive on
//! the main thread, through the OS event loop that winit runs.

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::i18n::tr;

const OPEN: &str = "oc-open";
const QUIT: &str = "oc-quit";

/// Keeps the icon in the tray.
pub struct Backend(#[allow(dead_code)] TrayIcon);

pub fn spawn(show: fn(), quit: fn()) -> Result<Backend, String> {
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| match e.id.0.as_str() {
        OPEN => show(),
        QUIT => quit(),
        _ => {}
    }));
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        // macOS opens the menu on a click (the menu bar convention); Windows shows the window on a left click.
        if cfg!(windows) {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show();
            }
        }
    }));
    let menu = Menu::new();
    let open = MenuItem::with_id(OPEN, tr("tray.open"), true, None);
    let quit_item = MenuItem::with_id(QUIT, tr("tray.quit"), true, None);
    menu.append_items(&[&open, &PredefinedMenuItem::separator(), &quit_item]).map_err(|e| e.to_string())?;
    let (px, w, h) = super::icon::rgba()?;
    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(cfg!(target_os = "macos"))
        .with_tooltip("OpenCompanion")
        .with_icon(Icon::from_rgba(px, w, h).map_err(|e| e.to_string())?)
        .build()
        .map(Backend)
        .map_err(|e| e.to_string())
}
