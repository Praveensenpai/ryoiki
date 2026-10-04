//! Persistent Seedr download queue with a single active slot.
//!
//! Seedr allows only one active cloud download at a time, so this queue is
//! strictly serial: at most one entry is `Active`, the rest wait.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Lifecycle state of a queued Seedr magnet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueueState {
    Queued,
    Active,
    Done,
    Failed,
}

/// Ordering policy for dequeuing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueuePolicy {
    Fifo,
    Lifo,
}

impl QueuePolicy {
    /// Parses a policy name, defaulting to FIFO for unknown input.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        if raw.eq_ignore_ascii_case("lifo") {
            Self::Lifo
        } else {
            Self::Fifo
        }
    }
}

/// A single magnet tracked by the Seedr queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub hash: String,
    pub magnet: String,
    pub name: String,
    pub seq: u64,
    pub state: QueueState,
    /// Unix seconds when the entry last became `Active`.
    #[serde(default)]
    pub activated_at: Option<u64>,
    /// Unix seconds when the active download first dropped below the slow threshold.
    #[serde(default)]
    pub slow_since: Option<u64>,
}

/// Current Unix time in seconds, or zero when the clock is before the epoch.
#[must_use]
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Persisted queue document.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SeedrQueue {
    #[serde(default)]
    pub entries: Vec<QueueEntry>,
    #[serde(default)]
    pub next_seq: u64,
}

impl SeedrQueue {
    /// Adds a magnet to the tail. Duplicate hashes are ignored.
    pub fn enqueue(&mut self, hash: &str, magnet: &str, name: &str) {
        if self.entries.iter().any(|e| e.hash == hash) {
            return;
        }
        self.next_seq += 1;
        self.entries.push(QueueEntry {
            hash: hash.to_string(),
            magnet: magnet.to_string(),
            name: name.to_string(),
            seq: self.next_seq,
            state: QueueState::Queued,
            activated_at: None,
            slow_since: None,
        });
    }

    /// Returns true when a download currently occupies the Seedr slot.
    #[must_use]
    pub fn has_active(&self) -> bool {
        self.entries.iter().any(|e| e.state == QueueState::Active)
    }

    /// Marks an entry active. Fails if another entry already holds the slot.
    pub fn activate(&mut self, hash: &str) -> bool {
        if self.entries.iter().any(|e| e.state == QueueState::Active) {
            return false;
        }
        let Some(entry) = self.entries.iter_mut().find(|e| e.hash == hash) else {
            return false;
        };
        entry.state = QueueState::Active;
        entry.activated_at = Some(now_secs());
        entry.slow_since = None;
        true
    }

    /// Returns the currently active entry, if any.
    #[must_use]
    pub fn active_entry(&self) -> Option<&QueueEntry> {
        self.entries.iter().find(|e| e.state == QueueState::Active)
    }

    /// Returns the active entry back to the queue, preserving its priority.
    ///
    /// The entry keeps its original `seq`, so under FIFO it resumes at the front.
    /// Used when a slow active download is preempted by a newer magnet.
    pub fn demote_to_queued(&mut self, hash: &str) -> bool {
        let min_seq = self.entries.iter().map(|e| e.seq).min().unwrap_or(0);
        let Some(entry) = self.entries.iter_mut().find(|e| e.hash == hash) else {
            return false;
        };
        entry.seq = min_seq.saturating_sub(1);
        entry.state = QueueState::Queued;
        entry.activated_at = None;
        entry.slow_since = None;
        true
    }

    /// Records the moment an active entry first dropped below the slow threshold.
    pub fn set_slow_since(&mut self, hash: &str, secs: u64) -> bool {
        let Some(entry) = self.entries.iter_mut().find(|e| e.hash == hash) else {
            return false;
        };
        if entry.state != QueueState::Active {
            return false;
        }
        if entry.slow_since.is_none() {
            entry.slow_since = Some(secs);
        }
        true
    }

    /// Clears a previously recorded slow period once speed recovers.
    pub fn clear_slow_since(&mut self, hash: &str) -> bool {
        let Some(entry) = self.entries.iter_mut().find(|e| e.hash == hash) else {
            return false;
        };
        entry.slow_since = None;
        true
    }

    /// Moves an entry to the front of the queue (next to run).
    pub fn promote_front(&mut self, hash: &str) -> bool {
        let min_seq = self.entries.iter().map(|e| e.seq).min().unwrap_or(0);
        let Some(entry) = self.entries.iter_mut().find(|e| e.hash == hash) else {
            return false;
        };
        entry.seq = min_seq.saturating_sub(1);
        entry.state = QueueState::Queued;
        true
    }

    /// Sets the terminal state for an entry.
    pub fn finish(&mut self, hash: &str, state: QueueState) -> bool {
        let Some(entry) = self.entries.iter_mut().find(|e| e.hash == hash) else {
            return false;
        };
        entry.state = state;
        true
    }

    /// Removes an entry entirely.
    pub fn remove(&mut self, hash: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.hash != hash);
        self.entries.len() != before
    }

    /// One-based queue position among pending entries, or zero when running/unknown.
    #[must_use]
    pub fn position(&self, hash: &str) -> usize {
        let mut pending: Vec<&QueueEntry> = self
            .entries
            .iter()
            .filter(|e| e.state == QueueState::Queued)
            .collect();
        pending.sort_by_key(|e| e.seq);
        pending
            .iter()
            .position(|e| e.hash == hash)
            .map_or(0, |i| i + 1)
    }

    /// Number of entries still waiting.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.state == QueueState::Queued)
            .count()
    }
}

fn state_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".local/share/ryoiki")
}

fn queue_path() -> PathBuf {
    state_dir().join("seedr_queue.json")
}

/// Serializes queue access for this process.
static INTRA_LOCK: Mutex<()> = Mutex::new(());

/// Lock-file path for a queue document (same stem, `.lock` extension).
fn lock_path_for(path: &Path) -> PathBuf {
    path.with_extension("lock")
}

/// Runs `f` while holding an exclusive lock on the queue file.
///
/// Both an in-process mutex and an advisory `flock` are taken so that the
/// webhook server, the torrent monitor, and a separate CLI process cannot
/// interleave read-modify-write cycles and lose updates.
pub fn with_lock<T>(f: impl FnOnce() -> T) -> T {
    with_lock_at(&queue_path(), f)
}

fn with_lock_at<T>(path: &Path, f: impl FnOnce() -> T) -> T {
    let _guard = INTRA_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let file = open_lock_file(&lock_path_for(path));
    if let Some(ref fh) = file {
        // Safety: `fh` is an open file descriptor for the lifetime of `file`.
        unsafe { libc::flock(fh.as_raw_fd(), libc::LOCK_EX) };
    }
    let result = f();
    // Dropping `file` releases the advisory lock.
    result
}

fn open_lock_file(path: &Path) -> Option<fs::File> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
        .ok()
}

/// Loads the persisted queue, returning an empty queue when absent or corrupt.
#[must_use]
pub fn load() -> SeedrQueue {
    load_at(&queue_path())
}

fn load_at(path: &Path) -> SeedrQueue {
    let Ok(raw) = fs::read_to_string(path) else {
        return SeedrQueue::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

/// Atomically persists the queue.
///
/// # Errors
/// Returns an error when the state directory cannot be created or written.
pub fn save(queue: &SeedrQueue) -> Result<()> {
    save_at(&queue_path(), queue)
}

fn save_at(path: &Path, queue: &SeedrQueue) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create ryoiki state directory")?;
    }
    let tmp = path.with_extension("tmp");
    let json = serde_json::to_string_pretty(queue).context("Failed to serialize Seedr queue")?;
    fs::write(&tmp, json).context("Failed to write Seedr queue")?;
    fs::rename(&tmp, path).context("Failed to replace Seedr queue")?;
    Ok(())
}

#[cfg(test)]
mod tests;
