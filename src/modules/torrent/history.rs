//! Persistent download history storage with file signature mapping.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::modules::media::MediaInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedFile {
    pub path: PathBuf,
    pub file_hash: String,
    pub size: u64,
    pub variant: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub hash: String,
    pub title: String,
    pub clean_name: String,
    #[serde(default)]
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub files: Vec<TrackedFile>,
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

pub fn compute_file_signature(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    len.hash(&mut hasher);

    let sample_size = 65536;
    let mut buf = vec![0u8; sample_size];

    let n = file.read(&mut buf)?;
    buf[..n].hash(&mut hasher);

    #[allow(clippy::cast_possible_wrap)]
    if len > (sample_size as u64) * 2 {
        let _ = file.seek(SeekFrom::End(-(sample_size as i64)));
        let n = file.read(&mut buf)?;
        buf[..n].hash(&mut hasher);
    }

    Ok(format!("{:016x}", hasher.finish()))
}

#[must_use]
pub fn create_tracked_file(path: PathBuf, variant: &str) -> TrackedFile {
    let (file_hash, size) = if let Ok(meta) = fs::metadata(&path) {
        let sz = meta.len();
        let sig = compute_file_signature(&path).unwrap_or_default();
        (sig, sz)
    } else {
        (String::new(), 0)
    };
    TrackedFile {
        path,
        file_hash,
        size,
        variant: variant.to_string(),
    }
}

pub fn record_download_history(
    hash: Option<&str>,
    info: &MediaInfo,
    files: Vec<TrackedFile>,
) -> Result<()> {
    let Some(h) = hash.filter(|s| !s.trim().is_empty()) else {
        return Ok(());
    };
    let mut history = load_history();
    let lower = h.to_lowercase();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    let paths: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    history.entries.insert(
        lower.clone(),
        HistoryRecord {
            hash: lower,
            title: info.title.clone(),
            clean_name: info.clean_name.clone(),
            paths,
            files,
            timestamp: now,
        },
    );

    let path = history_file_path()?;
    fs::write(path, serde_json::to_string_pretty(&history)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_file_signature_and_tracked_file() -> Result<()> {
        let tmp = std::env::temp_dir().join(format!("ryoiki_sig_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&tmp);
        let sample_file = tmp.join("sample.mkv");
        let sample_data = vec![42u8; 200_000];
        fs::write(&sample_file, &sample_data)?;

        let sig = compute_file_signature(&sample_file)?;
        assert_ne!(sig, "");

        let tracked = create_tracked_file(sample_file.clone(), "original");
        assert_eq!(tracked.size, 200_000);
        assert_eq!(tracked.file_hash, sig);
        assert_eq!(tracked.variant, "original");

        let _ = fs::remove_dir_all(&tmp);
        Ok(())
    }
}
