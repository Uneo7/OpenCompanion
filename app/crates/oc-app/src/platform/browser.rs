//! Opens a web page in the system browser, with the OS's own launcher (no dependency).

use std::process::Command;

/// Only `https://` links (a release page): never a local path or another scheme.
pub fn open_url(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err(format!("not an https link: {url}"));
    }
    let mut c = if cfg!(target_os = "windows") {
        let mut c = Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(url);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    c.spawn().map(drop).map_err(|e| format!("opening {url}: {e}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_https_links_open() {
        assert!(super::open_url("file:///etc/passwd").is_err());
        assert!(super::open_url("C:\\Windows\\notepad.exe").is_err());
    }
}
