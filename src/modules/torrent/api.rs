use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct TorrentInfo {
    pub name: String,
    pub total_size: i64,
    pub progress: f64,
    pub dlspeed: u64,
    pub upspeed: u64,
    pub eta: i64,
    pub state: String,
    pub category: String,
    pub has_metadata: bool,
    pub hash: String,
    #[serde(default)]
    pub content_path: Option<String>,
    #[serde(default)]
    pub save_path: Option<String>,
}

impl TorrentInfo {
    pub fn is_completed(&self) -> bool {
        self.progress >= 1.0
            || self.state == "pausedUP"
            || self.state == "stalledUP"
            || self.state == "uploading"
            || self.state == "forcedUP"
    }
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
    post_all(client, base_url, "stop", "pause")
}

pub fn resume_all(client: &Client, base_url: &str) -> Result<()> {
    post_all(client, base_url, "start", "resume")
}

fn post_all(client: &Client, base_url: &str, primary: &str, fallback: &str) -> Result<()> {
    let url = format!("{base_url}/api/v2/torrents/{primary}");
    let resp = client.post(&url).form(&[("hashes", "all")]).send()?;
    if resp.status().is_success() {
        return Ok(());
    }

    let fallback_url = format!("{base_url}/api/v2/torrents/{fallback}");
    let fb_resp = client
        .post(&fallback_url)
        .form(&[("hashes", "all")])
        .send()?;
    if !fb_resp.status().is_success() {
        bail!(
            "Failed to execute {primary}/{fallback}: HTTP {}",
            fb_resp.status()
        );
    }
    Ok(())
}

pub fn delete_torrent(
    client: &Client,
    base_url: &str,
    hash: &str,
    delete_files: bool,
) -> Result<()> {
    let url = format!("{base_url}/api/v2/torrents/delete");
    let delete_str = if delete_files { "true" } else { "false" };
    let resp = client
        .post(&url)
        .form(&[("hashes", hash), ("deleteFiles", delete_str)])
        .send()
        .with_context(|| format!("Failed to delete torrent from {url}"))?;

    if !resp.status().is_success() {
        bail!("Failed to delete torrent: HTTP {}", resp.status());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_torrent(progress: f64, state: &str) -> TorrentInfo {
        TorrentInfo {
            name: "Test Torrent".to_string(),
            total_size: 1000,
            progress,
            dlspeed: 0,
            upspeed: 0,
            eta: 0,
            state: state.to_string(),
            category: String::new(),
            has_metadata: true,
            hash: "testhash".to_string(),
            content_path: None,
            save_path: None,
        }
    }

    #[test]
    fn test_is_completed_progress_one() {
        let t = dummy_torrent(1.0, "pausedUP");
        assert!(t.is_completed());
    }

    #[test]
    fn test_is_completed_seeding_states() {
        let t1 = dummy_torrent(0.99, "uploading");
        assert!(t1.is_completed());
        let t2 = dummy_torrent(0.99, "stalledUP");
        assert!(t2.is_completed());
        let t3 = dummy_torrent(0.99, "pausedUP");
        assert!(t3.is_completed());
    }

    #[test]
    fn test_is_completed_downloading_false() {
        let t1 = dummy_torrent(0.5, "downloading");
        assert!(!t1.is_completed());
        let t2 = dummy_torrent(0.0, "missingFiles");
        assert!(!t2.is_completed());
        let t3 = dummy_torrent(0.99, "stalledDL");
        assert!(!t3.is_completed());
    }

    #[test]
    fn test_deserialize_meta_dl_negative_total_size() -> Result<()> {
        let json_str = r#"[
            {
                "name": "magnet_downloading_metadata",
                "total_size": -1,
                "progress": 0,
                "dlspeed": 0,
                "upspeed": 0,
                "eta": 8640000,
                "state": "metaDL",
                "category": "",
                "has_metadata": false,
                "hash": "8785008159dd93a6084924330ed4698701e78819",
                "content_path": "",
                "save_path": "/downloads"
            }
        ]"#;

        let torrents: Vec<TorrentInfo> = serde_json::from_str(json_str)?;
        assert_eq!(torrents.len(), 1);
        assert_eq!(torrents[0].total_size, -1);
        assert!(!torrents[0].has_metadata);
        assert!(!torrents[0].is_completed());
        Ok(())
    }

    #[test]
    fn test_deserialize_multiple_torrents_with_mixed_states() -> Result<()> {
        let json_str = r#"[
            {
                "name": "meta_dl_torrent",
                "total_size": -1,
                "progress": 0,
                "dlspeed": 0,
                "upspeed": 0,
                "eta": 8640000,
                "state": "metaDL",
                "category": "",
                "has_metadata": false,
                "hash": "8785008159dd93a6084924330ed4698701e78819",
                "content_path": "",
                "save_path": "/downloads"
            },
            {
                "name": "downloading_torrent",
                "total_size": 3806202719,
                "progress": 0.50191,
                "dlspeed": 102400,
                "upspeed": 51200,
                "eta": 1800,
                "state": "downloading",
                "category": "anime",
                "has_metadata": true,
                "hash": "dfb57c4bb32fc14086dc6f212d99422b9e7aa75a",
                "content_path": "/downloads/incomplete/test",
                "save_path": "/downloads"
            }
        ]"#;

        let torrents: Vec<TorrentInfo> = serde_json::from_str(json_str)?;
        assert_eq!(torrents.len(), 2);
        assert_eq!(torrents[0].total_size, -1);
        assert_eq!(torrents[1].total_size, 3_806_202_719);
        Ok(())
    }

    #[test]
    fn test_live_qbittorrent_if_running() {
        let client = Client::builder()
            .timeout(std::time::Duration::from_millis(500))
            .build()
            .unwrap_or_else(|_| Client::new());
        if let Ok(torrents) = get_torrents(&client, "http://localhost:6881", None) {
            println!("Fetched {} live torrents successfully!", torrents.len());
            assert!(!torrents.is_empty());
            for t in &torrents {
                assert!(!t.name.is_empty());
            }
        }
    }
}
