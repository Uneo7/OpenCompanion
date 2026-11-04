//! Linux: a StatusNotifierItem over D-Bus (KDE, GNOME with the AppIndicator extension, Xfce, Cinnamon…), pure
//! Rust. Its events arrive on ksni's own thread. No StatusNotifierWatcher on the session bus: no tray.

use ksni::blocking::TrayMethods;
use ksni::menu::StandardItem;
use ksni::{Icon, MenuItem};

use crate::i18n::tr;

/// Keeps the item registered.
pub struct Backend(#[allow(dead_code)] ksni::blocking::Handle<Item>);

pub struct Item {
    show: fn(),
    quit: fn(),
    icon: Icon,
}

impl ksni::Tray for Item {
    fn id(&self) -> String {
        "opencompanion".into()
    }

    fn title(&self) -> String {
        "OpenCompanion".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![self.icon.clone()]
    }

    /// A left click.
    fn activate(&mut self, _x: i32, _y: i32) {
        (self.show)();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem { label: tr("tray.open").into(), activate: Box::new(|t: &mut Self| (t.show)()), ..Default::default() }.into(),
            MenuItem::Separator,
            StandardItem { label: tr("tray.quit").into(), activate: Box::new(|t: &mut Self| (t.quit)()), ..Default::default() }.into(),
        ]
    }
}

pub fn spawn(show: fn(), quit: fn()) -> Result<Backend, String> {
    let (px, w, h) = super::icon::rgba()?;
    // ARGB32, network byte order.
    let data = px.chunks_exact(4).flat_map(|p| [p[3], p[0], p[1], p[2]]).collect();
    let icon = Icon { width: w as i32, height: h as i32, data };
    Item { show, quit, icon }.spawn().map(Backend).map_err(|e| e.to_string())
}
