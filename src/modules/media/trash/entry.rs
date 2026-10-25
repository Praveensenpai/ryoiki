//! Trash entry model and manifest persistence.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// How long a trashed entry is kept before it is purged for good.
pub const RETENTION_DAYS: u64 = 7;
const SECS_PER_DAY: u64 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrashKind {
    Title,
    Season,
    File,
}

impl TrashKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Season => "Season",
            Self::File => "File",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrashOrigin {
    Ssd,
    GDrive,
}

impl TrashOrigin {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Ssd => "SSD",
            Self::GDrive => "GDrive",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashEntry {
    pub id: String,
    pub kind: TrashKind,
    pub origin: TrashOrigin,
    pub title: String,
    pub size_bytes: u64,
    /// Original absolute path on the SSD, when the entry came from there.
    pub original_path: Option<PathBuf>,
    /// Original path relative to the cloud media root, when applicable.
    pub original_remote: Option<String>,
    /// Where the payload actually lives right now.
    pub trash_path: PathBuf,
    pub trashed_at: u64,
}

impl TrashEntry {
    #[must_use]
    pub fn expires_at(&self) -> u64 {
        self.trashed_at
            .saturating_add(RETENTION_DAYS * SECS_PER_DAY)
    }

    /// Whole days left before auto-purge; zero means it is due.
    #[must_use]
    pub fn days_left(&self) -> u64 {
        let now = now_secs();
        let expires = self.expires_at();
        if expires <= now {
            0
        } else {
            (expires - now).div_ceil(SECS_PER_DAY)
        }
    }

    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires_at() <= now_secs()
    }
}

/// Seconds since the Unix epoch.
#[must_use]
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `$XDG_STATE_HOME/ryoiki/media_trash.json`, falling back to `~/.local/state`.
///
/// `RYOIKI_STATE_DIR` takes precedence so tests can sandbox the manifest and
/// never touch the real profile.
#[must_use]
pub fn manifest_path() -> PathBuf {
    state_dir().join("ryoiki/media_trash.json")
}

fn state_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("RYOIKI_STATE_DIR") {
        return PathBuf::from(dir);
    }
    std::env::var_os("XDG_STATE_HOME").map_or_else(
        || {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            Path::new(&home).join(".local/state")
        },
        PathBuf::from,
    )
}

/// Loads every recorded trash entry, oldest first.
#[must_use]
pub fn load_entries() -> Vec<TrashEntry> {
    let Ok(raw) = fs::read_to_string(manifest_path()) else {
        return Vec::new();
    };
    let mut entries = serde_json::from_str::<Vec<TrashEntry>>(&raw).unwrap_or_default();
    entries.sort_by_key(|e| e.trashed_at);
    entries
}

/// Persists the full entry list, creating parent directories as needed.
pub fn save_entries(entries: &[TrashEntry]) -> Result<()> {
    let path = manifest_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create trash manifest dir {}", parent.display()))?;
    }
    let json =
        serde_json::to_string_pretty(entries).context("failed to serialize trash manifest")?;
    fs::write(&path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

/// Appends one entry and rewrites the manifest.
pub fn push_entry(entry: &TrashEntry) -> Result<()> {
    let mut entries = load_entries();
    entries.push(entry.clone());
    save_entries(&entries)
}

/// Removes one entry by id and rewrites the manifest.
pub fn remove_entry(id: &str) -> Result<()> {
    let mut entries = load_entries();
    entries.retain(|e| e.id != id);
    save_entries(&entries)
}

/// Root of the on-disk trash tree for the local SSD.
#[must_use]
pub fn local_trash_root(home: &Path) -> PathBuf {
    home.join("jellyfin/.trash/ssd")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(trashed_at: u64) -> TrashEntry {
        TrashEntry {
            id: "abc".into(),
            kind: TrashKind::Title,
            origin: TrashOrigin::Ssd,
            title: "Test".into(),
            size_bytes: 10,
            original_path: Some(PathBuf::from("/tmp/x")),
            original_remote: None,
            trash_path: PathBuf::from("/tmp/trash/abc"),
            trashed_at,
        }
    }

    #[test]
    fn days_left_counts_down_and_clamps_at_zero() {
        let fresh = entry(now_secs());
        assert_eq!(fresh.days_left(), RETENTION_DAYS);
        assert!(!fresh.is_expired());

        let stale = entry(now_secs().saturating_sub(RETENTION_DAYS * SECS_PER_DAY + 5));
        assert_eq!(stale.days_left(), 0);
        assert!(stale.is_expired());
    }
}
