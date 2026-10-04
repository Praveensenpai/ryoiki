//! Seedr task polling and formatting.

use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct SeedrTaskState {
    pub file_name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub speed_bps: u64,
    pub eta_seconds: u64,
    pub status: String,
}

#[derive(Deserialize)]
struct SeedrCloudList {
    #[serde(default)]
    torrents: Vec<SeedrCloudTorrent>,
}

#[derive(Deserialize)]
struct SeedrCloudTorrent {
    name: String,
    #[serde(default)]
    progress: Option<f64>,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    download_rate: Option<u64>,
}

#[must_use]
pub fn get_active_seedr_tasks() -> Vec<SeedrTaskState> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let tasks_dir = Path::new(&home).join(".cache/seedr-dl/tasks");
    let mut tasks = Vec::new();

    if let Ok(entries) = std::fs::read_dir(tasks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(task) = serde_json::from_str::<SeedrTaskState>(&content) {
                        tasks.push(task);
                    }
                }
            }
        }
    }

    if tasks.is_empty() {
        if let Ok(output) = std::process::Command::new("seedr-dl")
            .args(["list", "--json"])
            .output()
        {
            if output.status.success() {
                if let Ok(cloud) = serde_json::from_slice::<SeedrCloudList>(&output.stdout) {
                    for t in cloud.torrents {
                        let total = t.size.unwrap_or(0);
                        let pct = t.progress.unwrap_or(0.0);
                        #[allow(
                            clippy::cast_precision_loss,
                            clippy::cast_possible_truncation,
                            clippy::cast_sign_loss
                        )]
                        let downloaded = if total > 0 && pct > 0.0 {
                            ((pct / 100.0) * (total as f64)) as u64
                        } else {
                            0
                        };
                        let speed = t.download_rate.unwrap_or(0);
                        let eta = if speed > 0 && total > downloaded {
                            (total - downloaded) / speed
                        } else {
                            0
                        };
                        tasks.push(SeedrTaskState {
                            file_name: t.name,
                            downloaded_bytes: downloaded,
                            total_bytes: total,
                            speed_bps: speed,
                            eta_seconds: eta,
                            status: "Caching".to_string(),
                        });
                    }
                }
            }
        }
    }

    tasks
}

#[must_use]
pub fn format_seedr_tasks_section(tasks: &[SeedrTaskState]) -> String {
    if tasks.is_empty() {
        return String::new();
    }

    let mut lines = vec!["🌱 <b>Seedr Cloud Downloads:</b>".to_string()];
    for t in tasks {
        #[allow(clippy::cast_precision_loss)]
        let pct = if t.total_bytes > 0 {
            (t.downloaded_bytes as f64 / t.total_bytes as f64) * 100.0
        } else {
            0.0
        };
        let blocks = format!("{:.0}", pct / 10.0)
            .parse::<usize>()
            .unwrap_or(0)
            .min(10);
        let bar = format!(
            "[{}{}] {pct:.1}%",
            "█".repeat(blocks),
            "░".repeat(10 - blocks)
        );
        let dl_mb = t.downloaded_bytes / 1_048_576;
        let tot_mb = t.total_bytes / 1_048_576;
        #[allow(clippy::cast_precision_loss)]
        let spd = t.speed_bps as f64 / 1_048_576.0;
        let clean_name = crate::notify::client::escape_html(&t.file_name);
        let (icon, label) = if t.status.eq_ignore_ascii_case("caching") {
            ("☁️", "Caching in Seedr Cloud")
        } else if t.status.eq_ignore_ascii_case("organizing") {
            ("📁", "Organizing Media")
        } else {
            ("📥", "Downloading to Disk")
        };
        lines.push(format!(
            "{icon} <b>{clean_name}</b>\n<code>{bar}</code> • <b>{label}</b>\nSize: {dl_mb}/{tot_mb} MB | Rate: {spd:.2} MB/s | ETA: {}s\n",
            t.eta_seconds
        ));
    }
    lines.join("\n")
}
