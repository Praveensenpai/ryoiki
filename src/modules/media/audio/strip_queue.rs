use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{fs, time};

const MAX_ATTEMPTS: u8 = 24;
const QUEUE_FILENAME: &str = "pending_strips.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueEntry {
    pub path: PathBuf,
    pub attempts: u8,
    pub enqueued_secs: u64,
}

fn queue_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME env var not set")?;
    let dir = Path::new(&home).join(".local/share/ryoiki");
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create queue dir: {}", dir.display()))?;
    Ok(dir.join(QUEUE_FILENAME))
}

fn now_secs() -> u64 {
    time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn load() -> Result<Vec<QueueEntry>> {
    let path = queue_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read queue: {}", path.display()))?;
    serde_json::from_str(&raw).context("Failed to parse strip queue JSON")
}

fn save(entries: &[QueueEntry]) -> Result<()> {
    let path = queue_path()?;
    let json = serde_json::to_string_pretty(entries).context("Failed to serialize queue")?;
    fs::write(&path, json).with_context(|| format!("Failed to write queue: {}", path.display()))
}

/// Adds a path to the persistent retry queue if not already present.
pub fn enqueue(path: &Path) {
    let Ok(mut entries) = load() else { return };
    let already_queued = entries.iter().any(|e| e.path == path);
    if already_queued {
        return;
    }
    entries.push(QueueEntry {
        path: path.to_path_buf(),
        attempts: 0,
        enqueued_secs: now_secs(),
    });
    let _ = save(&entries);
    println!(
        "  {} Queued for hourly strip retry (up to {MAX_ATTEMPTS}h): {}",
        "⏱".cyan(),
        path.display()
    );
}

/// Processes the queue: retries dubstrip on each entry, removes successes and expired entries.
pub fn process_queue(dubstrip_bin: &Path) -> Result<()> {
    let mut entries = load()?;
    if entries.is_empty() {
        return Ok(());
    }

    println!(
        "\n  {} Processing strip retry queue ({} pending)...",
        "🗡️".cyan(),
        entries.len()
    );

    let mut remaining = Vec::new();
    for mut entry in entries.drain(..) {
        entry.attempts += 1;
        if try_strip(dubstrip_bin, &entry.path) {
            println!(
                "  {} Strip succeeded after {} attempt(s): {}",
                "✔".green().bold(),
                entry.attempts,
                entry.path.display()
            );
            super::sync_filename_after_strip(&entry.path);
        } else if entry.attempts >= MAX_ATTEMPTS {
            println!(
                "  {} Giving up after {MAX_ATTEMPTS} attempts: {}",
                "✖".red(),
                entry.path.display()
            );
        } else {
            remaining.push(entry);
        }
    }

    save(&remaining)?;
    println!(
        "  {} Queue processed. {} item(s) remaining.",
        "✔".green().bold(),
        remaining.len()
    );
    Ok(())
}

fn try_strip(bin: &Path, path: &Path) -> bool {
    Command::new(bin)
        .args(["strip", "--auto", "--force"])
        .arg(path)
        .status()
        .is_ok_and(|s| s.success())
}
