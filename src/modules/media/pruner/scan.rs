use anyhow::Result;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::PruneCandidate;

pub fn scan_media_candidates(base: &Path) -> Result<Vec<PruneCandidate>> {
    let mut list = Vec::new();
    let video_exts = ["mkv", "mp4", "avi", "mov", "m4v", "ts"];

    for category in &["movies", "shows"] {
        let cat_dir = base.join(category);
        if !cat_dir.exists() {
            continue;
        }
        collect_dir_candidates(&cat_dir, &video_exts, &mut list)?;
    }
    Ok(list)
}

fn collect_dir_candidates(dir: &Path, exts: &[&str], list: &mut Vec<PruneCandidate>) -> Result<()> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_dir_candidates(&path, exts, list)?;
            } else if path.is_file() {
                check_and_add_candidate(&path, exts, list);
            }
        }
    }
    Ok(())
}

fn check_and_add_candidate(path: &Path, exts: &[&str], list: &mut Vec<PruneCandidate>) {
    let matches_ext = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| exts.contains(&ext.to_ascii_lowercase().as_str()));

    if !matches_ext {
        return;
    }

    if let Ok(meta) = fs::metadata(path) {
        let size_bytes = meta.len();
        let modified = meta.modified().unwrap_or_else(|_| SystemTime::now());
        let name = path
            .file_name()
            .map_or("video".to_string(), |n| n.to_string_lossy().to_string());

        list.push(PruneCandidate {
            path: path.to_path_buf(),
            name,
            size_bytes,
            modified,
            is_watched: false,
        });
    }
}

pub fn tag_watched_status(candidates: &mut [PruneCandidate]) {
    let url = "http://localhost:8096/Items?Filters=IsPlayed&Recursive=true&Fields=Path";
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
        .ok();
    let resp = client.and_then(|c| c.get(url).send().ok());
    let json = resp.and_then(|r| r.json::<serde_json::Value>().ok());
    let Some(items) = json
        .as_ref()
        .and_then(|j| j.get("Items"))
        .and_then(|i| i.as_array())
    else {
        return;
    };
    for item in items {
        if let Some(p) = item.get("Path").and_then(|p| p.as_str()) {
            for c in candidates.iter_mut() {
                if c.path.to_string_lossy().contains(p) {
                    c.is_watched = true;
                }
            }
        }
    }
}
