//! Persistent download history storage.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::modules::media::MediaInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub hash: String,
    pub title: String,
    pub clean_name: String,
    pub paths: Vec<PathBuf>,
    pub timestamp: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DownloadHistory {
    pub entries: HashMap<String, HistoryRecord>,
}

fn history_file_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME env not set")?;
    let dir = Path::new(&home).join(".local/share/ryoiki");
    fs::create_dir_all(&dir)?;
    Ok(dir.join("download_history.json"))
}

#[must_use]
pub fn load_history() -> DownloadHistory {
    let Ok(path) = history_file_path() else {
        return DownloadHistory::default();
    };
    if !path.exists() {
        return DownloadHistory::default();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn record_download_history(
    hash: Option<&str>,
    info: &MediaInfo,
    paths: &[PathBuf],
) -> Result<()> {
    let Some(h) = hash.filter(|s| !s.trim().is_empty()) else {
        return Ok(());
    };
    let mut history = load_history();
    let lower = h.to_lowercase();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    history.entries.insert(
        lower.clone(),
        HistoryRecord {
            hash: lower,
            title: info.title.clone(),
            clean_name: info.clean_name.clone(),
            paths: paths.to_vec(),
            timestamp: now,
        },
    );

    let path = history_file_path()?;
    fs::write(path, serde_json::to_string_pretty(&history)?)?;
    Ok(())
}
