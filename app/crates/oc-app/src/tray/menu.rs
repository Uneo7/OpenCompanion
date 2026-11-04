//! The tray icon and its menu: Open OpenCompanion, Quit.

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::i18n::tr;

const OPEN: &str = "oc-open";
const QUIT: &str = "oc-quit";
/// 64 px: sharp on HiDPI notification areas and the Retina menu bar; the OS scales it down.
const PNG: &[u8] = include_bytes!("../../../../packaging/icons/png/opencompanion-64.png");

pub fn on_events(show: fn(), quit: fn()) {
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| match e.id.0.as_str() {
        OPEN => show(),
        QUIT => quit(),
        _ => {}
    }));
}

pub fn build() -> Result<TrayIcon, String> {
    let menu = Menu::new();
    let open = MenuItem::with_id(OPEN, tr("tray.open"), true, None);
    let quit = MenuItem::with_id(QUIT, tr("tray.quit"), true, None);
    menu.append_items(&[&open, &PredefinedMenuItem::separator(), &quit]).map_err(|e| e.to_string())?;
    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(cfg!(target_os = "macos"))
        .with_tooltip("OpenCompanion")
        .with_icon(icon()?)
        .build()
        .map_err(|e| e.to_string())
}

fn icon() -> Result<Icon, String> {
    let img = image::load_from_memory_with_format(PNG, image::ImageFormat::Png).map_err(|e| e.to_string())?.to_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_tray_icon_decodes() {
        assert!(super::icon().is_ok());
    }
}
