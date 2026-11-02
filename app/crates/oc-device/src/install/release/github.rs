//! No token: 60 requests per hour and per address, enough for one check per connection.

use std::io::Read;
use std::time::Duration;

use serde::Deserialize;

use super::manifest::{Manifest, MANIFEST_NAME};

const USER_AGENT: &str = concat!("OpenCompanion/", env!("CARGO_PKG_VERSION"));
const TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
    /// `sha256:<hex>` (given by the API for recent uploads).
    #[serde(default)]
    pub digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Available {
    pub tag: String,
    pub manifest: Manifest,
    pub zip: Asset,
    /// Expected SHA-256 of the zip (from the `.sha256` asset, else the API's `digest`).
    pub zip_sha256: Option<String>,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(TIMEOUT).user_agent(USER_AGENT).build()
}

fn get(url: &str) -> Result<ureq::Response, String> {
    agent().get(url).set("Accept", "application/vnd.github+json").call().map_err(|e| match e {
        ureq::Error::Status(403 | 429, _) => "GitHub rate limit reached".to_owned(),
        ureq::Error::Status(404, _) => "no release published".to_owned(),
        e => e.to_string(),
    })
}

fn get_text(url: &str) -> Result<String, String> {
    get(url)?.into_string().map_err(|e| e.to_string())
}

pub fn latest(api_base: &str, repo: &str) -> Result<Release, String> {
    let body = get_text(&format!("{api_base}/repos/{repo}/releases/latest"))?;
    serde_json::from_str(&body).map_err(|e| format!("release JSON: {e}"))
}

pub fn available(release: &Release) -> Result<Available, String> {
    let zip = release
        .assets
        .iter()
        .find(|a| a.name.starts_with("opencompanion-firmware-") && a.name.ends_with(".zip"))
        .ok_or_else(|| format!("release {}: no firmware zip", release.tag_name))?
        .clone();
    let manifest_asset = release
        .assets
        .iter()
        .find(|a| a.name == MANIFEST_NAME)
        .ok_or_else(|| format!("release {}: no {MANIFEST_NAME}", release.tag_name))?;
    let manifest = Manifest::parse(&get_text(&manifest_asset.browser_download_url)?)?;
    let sha_asset = release.assets.iter().find(|a| a.name == format!("{}.sha256", zip.name));
    let zip_sha256 = match sha_asset {
        Some(a) => get_text(&a.browser_download_url)?.split_whitespace().next().map(str::to_ascii_lowercase),
        None => zip.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")).map(str::to_ascii_lowercase),
    };
    Ok(Available { tag: release.tag_name.clone(), manifest, zip, zip_sha256 })
}

pub fn download(asset: &Asset, mut progress: impl FnMut(u64, u64)) -> Result<Vec<u8>, String> {
    let mut reader = get(&asset.browser_download_url)?.into_reader();
    let total = asset.size.max(1);
    let mut out = Vec::with_capacity(asset.size as usize);
    let mut buf = [0u8; 16 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("download: {e}"))?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
        progress(out.len() as u64, total);
    }
    if out.len() as u64 != asset.size {
        return Err(format!("download: {} bytes, expected {}", out.len(), asset.size));
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    use super::*;

    pub fn serve(routes: Vec<(String, u16, Vec<u8>)>) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in l.incoming().flatten() {
                let mut s = stream;
                let mut first = String::new();
                let mut r = BufReader::new(s.try_clone().unwrap());
                if r.read_line(&mut first).is_err() {
                    continue;
                }
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).is_err() || h == "\r\n" || h.is_empty() {
                        break;
                    }
                }
                let path = first.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let (status, body) = routes.iter().find(|(p, ..)| *p == path).map_or((404, b"{}".to_vec()), |(_, s, b)| (*s, b.clone()));
                let _ = write!(s, "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = s.write_all(&body);
            }
        });
        base
    }

    #[test]
    fn latest_release_manifest_and_download() {
        let m = super::super::manifest::Manifest {
            format: "opencompanion-firmware".into(),
            manifest_version: 1,
            dsp_version: "0.1.0+1".into(),
            tas_version: "0.4.1".into(),
            min_app_version: "0.1.0".into(),
            files: super::super::manifest::FILES
                .iter()
                .map(|p| super::super::manifest::FileEntry { path: (*p).into(), size: 1, sha256: "0".repeat(64) })
                .collect(),
        };
        let zip = b"zip bytes".to_vec();
        let zip_sha = super::super::manifest::sha256_hex(&zip);
        // The release JSON needs the server's address: two servers, assets first.
        let assets = serve(vec![
            ("/m".into(), 200, m.to_json().into_bytes()),
            ("/z".into(), 200, zip.clone()),
            ("/s".into(), 200, format!("{zip_sha}  opencompanion-firmware-0.1.0-0.4.1.zip\n").into_bytes()),
        ]);
        let release = format!(
            r#"{{"tag_name":"v0.1.0","assets":[
                {{"name":"firmware-manifest.json","browser_download_url":"{assets}/m","size":1}},
                {{"name":"opencompanion-firmware-0.1.0-0.4.1.zip","browser_download_url":"{assets}/z","size":{}}},
                {{"name":"opencompanion-firmware-0.1.0-0.4.1.zip.sha256","browser_download_url":"{assets}/s","size":1}}]}}"#,
            zip.len()
        );
        let api = serve(vec![
            ("/repos/o/r/releases/latest".into(), 200, release.into_bytes()),
            ("/repos/o/limited/releases/latest".into(), 403, b"{}".to_vec()),
        ]);
        let rel = latest(&api, "o/r").unwrap();
        assert_eq!(rel.tag_name, "v0.1.0");
        let a = available(&rel).unwrap();
        assert_eq!((a.manifest.dsp_version.as_str(), a.zip_sha256.as_deref()), ("0.1.0+1", Some(zip_sha.as_str())));
        let mut seen = Vec::new();
        assert_eq!(download(&a.zip, |d, t| seen.push((d, t))).unwrap(), zip);
        assert_eq!(seen.last(), Some(&(zip.len() as u64, zip.len() as u64)));
        assert_eq!(latest(&api, "o/limited").unwrap_err(), "GitHub rate limit reached");
        assert!(latest("http://127.0.0.1:9", "o/r").is_err(), "offline");
    }
}
