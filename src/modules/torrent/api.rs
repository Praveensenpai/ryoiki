use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct TorrentInfo {
    pub name: String,
    pub total_size: u64,
    pub progress: f64,
    pub dlspeed: u64,
    pub upspeed: u64,
    pub eta: i64,
    pub state: String,
    pub category: String,
    pub has_metadata: bool,
    pub hash: String,
}

pub fn get_torrents(
    client: &Client,
    base_url: &str,
    hash: Option<&str>,
) -> Result<Vec<TorrentInfo>> {
    let url = match hash {
        Some(h) if !h.is_empty() => format!("{base_url}/api/v2/torrents/info?hashes={h}"),
        _ => format!("{base_url}/api/v2/torrents/info"),
    };

    let resp = client
        .get(&url)
        .send()
        .with_context(|| format!("Failed to reach qBittorrent at {url}"))?;

    if !resp.status().is_success() {
        bail!("qBittorrent returned error HTTP {}", resp.status());
    }

    let items: Vec<TorrentInfo> = resp
        .json()
        .context("Failed to parse qBittorrent torrents JSON")?;
    Ok(items)
}

pub fn add_magnet(client: &Client, base_url: &str, magnet: &str) -> Result<()> {
    let url = format!("{base_url}/api/v2/torrents/add");
    let resp = client
        .post(&url)
        .form(&[("urls", magnet)])
        .send()
        .with_context(|| format!("Failed to add magnet to {url}"))?;

    if !resp.status().is_success() {
        bail!("Failed to add magnet: HTTP {}", resp.status());
    }
    Ok(())
}

pub fn add_torrent_file(
    client: &Client,
    base_url: &str,
    filename: &str,
    bytes: Vec<u8>,
) -> Result<()> {
    let url = format!("{base_url}/api/v2/torrents/add");
    let part = reqwest::blocking::multipart::Part::bytes(bytes)
        .file_name(filename.to_string())
        .mime_str("application/x-bittorrent")
        .context("Invalid mime type")?;

    let form = reqwest::blocking::multipart::Form::new().part("torrents", part);
    let resp = client
        .post(&url)
        .multipart(form)
        .send()
        .with_context(|| format!("Failed to upload torrent file to {url}"))?;

    if !resp.status().is_success() {
        bail!("Failed to add torrent file: HTTP {}", resp.status());
    }
    Ok(())
}

pub fn pause_all(client: &Client, base_url: &str) -> Result<()> {
    let url = format!("{base_url}/api/v2/torrents/pause");
    let resp = client.post(&url).form(&[("hashes", "all")]).send()?;
    if !resp.status().is_success() {
        bail!("Failed to pause torrents: HTTP {}", resp.status());
    }
    Ok(())
}

pub fn resume_all(client: &Client, base_url: &str) -> Result<()> {
    let url = format!("{base_url}/api/v2/torrents/resume");
    let resp = client.post(&url).form(&[("hashes", "all")]).send()?;
    if !resp.status().is_success() {
        bail!("Failed to resume torrents: HTTP {}", resp.status());
    }
    Ok(())
}
