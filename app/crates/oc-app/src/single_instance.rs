//! One app at a time: two instances would fight over the speaker's HID interface (exclusive on macOS) and the
//! OS volume. The first instance listens on a loopback port; a second launch asks it to come to the front, then
//! exits. Same code on Windows, macOS and Linux, no dependency.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use eframe::egui::{Context, ViewportCommand};

const PORT: u16 = 47_651;
const HELLO: &str = "OpenCompanion show";
const ACK: &str = "OpenCompanion ok";
const TIMEOUT: Duration = Duration::from_millis(800);

static SHOW: AtomicBool = AtomicBool::new(false);
static CTX: OnceLock<Context> = OnceLock::new();

pub enum Instance {
    /// This process runs the app (the listener lives as long as the process).
    First,
    /// Another OpenCompanion is running and was asked to come to the front.
    Other,
}

/// A port held by something that is not OpenCompanion never blocks the launch.
pub fn acquire() -> Instance {
    acquire_on(PORT)
}

fn acquire_on(port: u16) -> Instance {
    match TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
        Ok(l) => {
            let _ = std::thread::Builder::new().name("oc-single-instance".into()).spawn(move || serve(&l));
            Instance::First
        }
        Err(_) if ask_first(port) => Instance::Other,
        Err(_) => Instance::First,
    }
}

fn serve(l: &TcpListener) {
    for s in l.incoming().flatten() {
        let _ = s.set_read_timeout(Some(TIMEOUT));
        let mut line = String::new();
        if BufReader::new(&s).read_line(&mut line).is_ok() && line.trim() == HELLO {
            let _ = writeln!(&s, "{ACK}");
            SHOW.store(true, Ordering::SeqCst);
            // The window may be hidden in the tray.
            crate::tray::request_show();
            if let Some(ctx) = CTX.get() {
                ctx.request_repaint();
            }
        }
    }
}

fn ask_first(port: u16) -> bool {
    let Ok(s) = TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), TIMEOUT) else { return false };
    let _ = s.set_read_timeout(Some(TIMEOUT));
    if writeln!(&s, "{HELLO}").is_err() {
        return false;
    }
    let mut line = String::new();
    BufReader::new(&s).read_line(&mut line).is_ok() && line.trim() == ACK
}

/// Every frame: a second launch brings this window to the front.
pub fn watch(ctx: &Context) {
    let _ = CTX.set(ctx.clone());
    if SHOW.swap(false, Ordering::SeqCst) {
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_instance_hands_over_to_the_first() {
        let port = 47_700 + (std::process::id() % 200) as u16;
        assert!(matches!(acquire_on(port), Instance::First));
        assert!(matches!(acquire_on(port), Instance::Other));
        // The listener thread sets the flag before acknowledging.
        assert!(SHOW.swap(false, Ordering::SeqCst));
        // A port held by another program: launch anyway.
        let other = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let busy = other.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for s in other.incoming().flatten() {
                drop(s);
            }
        });
        assert!(matches!(acquire_on(busy), Instance::First));
    }
}
