//! Persistent store for pending original-language decisions and their history.
//!
//! After a multi-audio file is staged, ryoiki asks which track is the true
//! original. The decision stays pending until the user picks a language (or the
//! 7-day grace window expires), and the resolved choice is kept as history so
//! the same title is never asked about twice.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::{fs, time};

/// Days a decision may stay pending before the AI recommendation is applied.
pub const PENDING_DAYS: u64 = 7;

const STORE_FILENAME: &str = "audio-decisions.json";
/// Seconds in one day, shared with the grace-window maths.
pub const SECS_PER_DAY: u64 = 86_400;
/// Reminder cadence: one prompt per pending title every hour.
pub const SECS_PER_HOUR: u64 = 3_600;

/// A language choice that is waiting on the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingDecision {
    /// Callback-safe identifier derived from the title key.
    pub id: String,
    /// Display filename of the staged (stripped) file.
    pub title: String,
    /// Path of the `--auto` stripped file currently on disk.
    pub stripped_path: PathBuf,
    /// Path of the full `[Multi]` sidecar that holds every audio track.
    pub multi_path: PathBuf,
    /// AI-recommended original language, once computed.
    #[serde(default)]
    pub recommended: Option<String>,
    /// Languages available in the `[Multi]` sidecar.
    #[serde(default)]
    pub candidates: Vec<String>,
    /// Epoch seconds when the decision was staged.
    pub created_secs: u64,
    /// Epoch seconds of the last reminder sent.
    #[serde(default)]
    pub last_reminded_secs: u64,
    /// Telegram message id of the last prompt, when one was sent.
    #[serde(default)]
    pub prompt_msg_id: Option<i64>,
}

/// A resolved language decision, used to deduplicate repeat titles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub id: String,
    pub title: String,
    pub chosen: String,
    pub decided_secs: u64,
    /// True when the 7-day grace window applied the recommendation.
    #[serde(default)]
    pub auto: bool,
    /// True when the user asked to stop being prompted. The `[Multi]` file is
    /// left untouched and the title is never queued again.
    #[serde(default)]
    pub silenced: bool,
}

/// On-disk collection of pending decisions and resolved history.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DecisionStore {
    #[serde(default)]
    pub pending: Vec<PendingDecision>,
    #[serde(default)]
    pub history: Vec<DecisionRecord>,
}

#[must_use]
pub fn now_secs() -> u64 {
    time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn store_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME env var not set")?;
    let dir = Path::new(&home).join(".local/share/ryoiki");
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create store dir: {}", dir.display()))?;
    Ok(dir.join(STORE_FILENAME))
}

pub fn load() -> Result<DecisionStore> {
    let path = store_path()?;
    if !path.exists() {
        return Ok(DecisionStore::default());
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read decisions: {}", path.display()))?;
    serde_json::from_str(&raw).context("Failed to parse audio decisions JSON")
}

pub fn save(store: &DecisionStore) -> Result<()> {
    let path = store_path()?;
    let json = serde_json::to_string_pretty(store).context("Failed to serialize decisions")?;
    fs::write(&path, json).with_context(|| format!("Failed to write decisions: {}", path.display()))
}

/// Stable identity for a title: the filename stem with every bracketed or
/// parenthesised tag removed, lowercased and whitespace-collapsed.
///
/// This makes `Show S01E01 [Multi] [1080p].mkv` and `Show S01E01 [Telugu].mkv`
/// share one key, so the same title is never asked about twice.
#[must_use]
pub fn decision_key(name: &str) -> String {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    let mut out = String::with_capacity(stem.len());
    let mut depth = 0u32;
    for c in stem.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Short, callback-safe identifier derived from [`decision_key`].
#[must_use]
pub fn short_id(key: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let hex = format!("{hash:016x}");
    hex[..12].to_string()
}

/// Whole days a pending decision has been waiting.
#[must_use]
pub fn days_pending(entry: &PendingDecision) -> u64 {
    now_secs().saturating_sub(entry.created_secs) / SECS_PER_DAY
}

/// Days left before the recommendation is applied automatically.
#[must_use]
pub fn days_left(entry: &PendingDecision) -> u64 {
    PENDING_DAYS.saturating_sub(days_pending(entry))
}

#[must_use]
pub fn find_history<'a>(store: &'a DecisionStore, id: &str) -> Option<&'a DecisionRecord> {
    store.history.iter().find(|r| r.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silenced_flag_roundtrips() {
        let json =
            r#"{"id":"a","title":"t","chosen":"kept [Multi]","decided_secs":1,"silenced":true}"#;
        let record: DecisionRecord = serde_json::from_str(json).unwrap_or_else(|e| panic!("{e}"));
        assert!(record.silenced);
        assert!(!record.auto);

        // Older records without the field must still load as not silenced.
        let legacy = r#"{"id":"a","title":"t","chosen":"Tamil","decided_secs":1}"#;
        let old: DecisionRecord = serde_json::from_str(legacy).unwrap_or_else(|e| panic!("{e}"));
        assert!(!old.silenced);
    }

    #[test]
    fn test_decision_key_strips_tags() {
        let a = decision_key("Show S01E01 [Multi] [1080p].mkv");
        let b = decision_key("Show S01E01 [Telugu] [1080p].mkv");
        assert_eq!(a, b);
        assert_eq!(a, "show s01e01");
    }

    #[test]
    fn test_decision_key_keeps_title_words() {
        assert_eq!(
            decision_key("The.Movie.2024.1080p.mkv"),
            "the.movie.2024.1080p"
        );
    }

    #[test]
    fn test_short_id_is_stable_and_short() {
        let id = short_id("show s01e01");
        assert_eq!(id.len(), 12);
        assert_eq!(id, short_id("show s01e01"));
        assert_ne!(id, short_id("show s01e02"));
    }
}
