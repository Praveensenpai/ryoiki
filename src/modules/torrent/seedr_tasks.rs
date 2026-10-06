//! Seedr task polling, formatting, and stale-state reconciliation.

use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct SeedrTaskState {
    pub file_name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub speed_bps: u64,
    pub eta_seconds: u64,
    pub status: String,
    #[serde(default)]
    pub pid: Option<u32>,
}

#[derive(Deserialize)]
pub(crate) struct SeedrCloudList {
    #[serde(default)]
    pub(crate) torrents: Vec<SeedrCloudTorrent>,
    #[serde(default)]
    pub(crate) folders: Vec<SeedrCloudNamed>,
    #[serde(default)]
    pub(crate) files: Vec<SeedrCloudNamed>,
    #[serde(default)]
    pub(crate) space_max: Option<u64>,
    #[serde(default)]
    pub(crate) space_used: Option<u64>,
}

/// Remaining Seedr cloud space in bytes, or `None` when the account is unreachable.
#[must_use]
pub fn seedr_available_bytes() -> Option<u64> {
    let list = fetch_live_list()?;
    let max = list.space_max?;
    let used = list.space_used.unwrap_or(0);
    Some(max.saturating_sub(used))
}

#[derive(Deserialize)]
pub(crate) struct SeedrCloudNamed {
    #[serde(default)]
    pub(crate) id: u64,
    pub(crate) name: String,
}

#[derive(Deserialize)]
pub(crate) struct SeedrCloudTorrent {
    #[serde(default)]
    pub(crate) id: u64,
    pub(crate) name: String,
    #[serde(default)]
    progress: Option<f64>,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    download_rate: Option<u64>,
}

/// Decision on whether a locally-tracked task should survive reconciliation.
enum TaskVerdict {
    Keep,
    Prune,
}

pub(crate) fn tasks_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    Path::new(&home).join(".cache/seedr-dl/tasks")
}

/// Loads all local task JSON files paired with their on-disk paths.
pub(crate) fn load_local_tasks(dir: &Path) -> Vec<(PathBuf, SeedrTaskState)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(task) = serde_json::from_str::<SeedrTaskState>(&content) {
                out.push((path, task));
            }
        }
    }
    out
}

/// Queries the Seedr CLI once. Returns `None` on any failure.
fn query_seedr_once() -> Option<SeedrCloudList> {
    let output = std::process::Command::new("seedr-dl")
        .args(["list", "--json"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice::<SeedrCloudList>(&output.stdout).ok()
}

/// Queries Seedr with bounded retries so transient API blips never prune state.
pub(crate) fn fetch_live_list() -> Option<SeedrCloudList> {
    const BACKOFF_SECS: [u64; 2] = [1, 2];
    for attempt in 0..=BACKOFF_SECS.len() {
        if let Some(list) = query_seedr_once() {
            return Some(list);
        }
        if let Some(&delay) = BACKOFF_SECS.get(attempt) {
            std::thread::sleep(std::time::Duration::from_secs(delay));
        }
    }
    None
}

pub(crate) fn normalize(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Set of names currently present in the Seedr cloud account.
struct LiveIndex {
    names: HashSet<String>,
}

impl LiveIndex {
    fn from_list(list: &SeedrCloudList) -> Self {
        let mut names = HashSet::new();
        for t in &list.torrents {
            names.insert(normalize(&t.name));
        }
        for f in &list.folders {
            names.insert(normalize(&f.name));
        }
        for f in &list.files {
            names.insert(normalize(&f.name));
        }
        Self { names }
    }

    fn contains(&self, name: &str) -> bool {
        if self.names.contains(&normalize(name)) {
            return true;
        }
        name.strip_prefix("folder-")
            .is_some_and(|stripped| self.names.contains(&normalize(stripped)))
    }
}

/// Returns true when the background worker process is still alive.
fn worker_alive(pid: Option<u32>) -> bool {
    pid.is_some_and(|p| Path::new(&format!("/proc/{p}")).exists())
}

/// Whether a live Seedr cloud entry matches this name.
///
/// `None` means the cloud list could not be fetched (never prune on a blip).
#[must_use]
pub(crate) fn cloud_contains(name: &str) -> Option<bool> {
    let list = fetch_live_list()?;
    Some(LiveIndex::from_list(&list).contains(name))
}

/// Whether any local task matching this name has a live worker process.
#[must_use]
pub(crate) fn worker_alive_by_name(name: &str) -> bool {
    let target = normalize(name);
    load_local_tasks(&tasks_dir()).iter().any(|(_, task)| {
        let candidate = normalize(&task.file_name);
        (candidate == target || candidate.contains(&target) || target.contains(&candidate))
            && worker_alive(task.pid)
    })
}

/// Decides whether a local task should be kept or pruned.
fn verdict(task: &SeedrTaskState, index: &LiveIndex) -> TaskVerdict {
    if index.contains(&task.file_name) || worker_alive(task.pid) {
        TaskVerdict::Keep
    } else {
        TaskVerdict::Prune
    }
}

/// Alerts the user before a stale task's local state is removed.
fn notify_stale(task: &SeedrTaskState) {
    use crate::notify::client::{format_card, send_alert};

    let Ok(config) = crate::notify::config::TelegramConfig::load() else {
        return;
    };
    let name = crate::notify::client::escape_html(&task.file_name);
    let size = format!("{} MB", task.total_bytes / 1_048_576);
    let card = format_card(
        "Seedr",
        "🧹 <b>STALE SEEDR TASK REMOVED</b>",
        &[
            ("File:", &name),
            ("Size:", &size),
            ("Reason:", "No longer present in Seedr cloud"),
        ],
    );
    let _ = send_alert(&config.bot_token, &config.chat_id, &card);
}

#[must_use]
pub fn get_active_seedr_tasks() -> Vec<SeedrTaskState> {
    let dir = tasks_dir();
    let local = load_local_tasks(&dir);
    let live = fetch_live_list();

    if local.is_empty() {
        return live.as_ref().map_or_else(Vec::new, synthesize_from_live);
    }

    // Network trouble: keep local state intact rather than guessing.
    let Some(live) = live else {
        return local.into_iter().map(|(_, task)| task).collect();
    };

    let index = LiveIndex::from_list(&live);
    let mut kept = Vec::with_capacity(local.len());
    for (path, task) in local {
        match verdict(&task, &index) {
            TaskVerdict::Keep => kept.push(task),
            TaskVerdict::Prune => {
                notify_stale(&task);
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    kept
}

/// Builds display tasks from the live Seedr list when no local state exists.
fn synthesize_from_live(list: &SeedrCloudList) -> Vec<SeedrTaskState> {
    list.torrents.iter().map(task_from_live).collect()
}

// reason: sizes are byte counts well under 2^52; f64 keeps exact integers at this scale.
#[allow(clippy::cast_precision_loss)]
fn task_from_live(t: &SeedrCloudTorrent) -> SeedrTaskState {
    let total = t.size.unwrap_or(0);
    let pct = t.progress.unwrap_or(0.0);
    let downloaded = if total > 0 && pct > 0.0 {
        cast_u64((pct / 100.0) * (total as f64))
    } else {
        0
    };
    let speed = t.download_rate.unwrap_or(0);
    let eta = if speed > 0 && total > downloaded {
        (total - downloaded) / speed
    } else {
        0
    };
    SeedrTaskState {
        file_name: t.name.clone(),
        downloaded_bytes: downloaded,
        total_bytes: total,
        speed_bps: speed,
        eta_seconds: eta,
        status: "Caching".to_string(),
        pid: None,
    }
}

// reason: Seedr progress is a non-negative percentage; truncation is fine for display.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn cast_u64(value: f64) -> u64 {
    value as u64
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

#[cfg(test)]
mod tests;
