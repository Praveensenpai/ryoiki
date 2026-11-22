//! Original-language approval: AI recommendation, Telegram prompt, and the
//! ffmpeg remux that reduces the `[Multi]` sidecar to one chosen track.

use anyhow::Result;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::path::{Path, PathBuf};

use super::super::probe::{self, MediaProbe};
use super::decisions::{self, find_history, now_secs, DecisionRecord, PendingDecision};
use super::remux::apply_language;

const MAX_CANDIDATES: usize = 6;

mod notify;
mod scan;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub use notify::render_prompt;
pub use notify::{list_pending_text, notify_pending};
pub use scan::{scan_library, silence};

#[derive(Debug, Deserialize)]
struct LangVerdict {
    language: Option<String>,
}

/// Asks the configured AI which language is the true original.
#[must_use]
pub fn recommend_language(
    client: &Client,
    raw_name: &str,
    probe: Option<&MediaProbe>,
) -> Option<String> {
    let langs = probe.map(|p| p.audio_languages.clone()).unwrap_or_default();
    let prompt = build_prompt(raw_name, &langs);
    let raw = crate::modules::media::ai::send_ai_prompt(client, None, &prompt).ok()?;
    let parsed: LangVerdict = serde_json::from_str(&raw).ok()?;
    parsed
        .language
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
}

fn build_prompt(raw_name: &str, langs: &[String]) -> String {
    let list = if langs.is_empty() {
        "unknown".to_string()
    } else {
        langs.join(", ")
    };
    format!(
        "You identify the ORIGINAL language of a film or TV episode from its release name.\n\
         Release name: {raw_name}\n\
         Available audio tracks: {list}\n\
         Reply with strict JSON only: {{\"language\": \"<one of the available tracks>\"}}.\n\
         Choose the language the title was originally produced in."
    )
}

/// Candidates for the inline keyboard, recommended language first.
fn build_candidates(probe: Option<&MediaProbe>, recommended: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(rec) = recommended {
        out.push(rec.to_string());
    }
    if let Some(p) = probe {
        for l in &p.audio_languages {
            if !out.iter().any(|c| c.eq_ignore_ascii_case(l)) {
                out.push(l.clone());
            }
        }
    }
    if out.is_empty() {
        out.push("Original".to_string());
    }
    out.truncate(MAX_CANDIDATES);
    out
}

/// Result of staging a file for a language decision.
pub enum StageOutcome {
    /// The decision is now pending; the user will be prompted.
    Pending,
    /// The title was already decided; carries the human-readable reason.
    Duplicate(String),
    /// The file carries no genuine language choice (one distinct language).
    Unnecessary,
    /// The decision could not be persisted (store error).
    Failed,
}

/// Records a staged file as awaiting a language decision.
///
/// Returns [`StageOutcome::Duplicate`] when the title already has a resolved
/// decision, so the caller can report it instead of prompting again.
pub fn register(
    stripped_path: &Path,
    multi_path: &Path,
    probe: Option<&MediaProbe>,
    recommended: Option<&str>,
) -> StageOutcome {
    let Some(name) = stripped_path.file_name().and_then(|n| n.to_str()) else {
        return StageOutcome::Failed;
    };
    let key = decisions::decision_key(name);
    let id = decisions::short_id(&key);

    let Ok(mut store) = decisions::load() else {
        return StageOutcome::Failed;
    };

    if let Some(rec) = find_history(&store, &id) {
        return StageOutcome::Duplicate(if rec.silenced {
            "prompting silenced for this title — keeping the [Multi] copy".to_string()
        } else {
            format!("already selected as [{}] — skipping duplicate", rec.chosen)
        });
    }

    if let Some(existing) = store.pending.iter_mut().find(|p| p.id == id) {
        existing.stripped_path = stripped_path.to_path_buf();
        existing.multi_path = multi_path.to_path_buf();
        existing.title = name.to_string();
        let _ = decisions::save(&store);
        return StageOutcome::Pending;
    }

    // A choice only exists when the file carries more than one distinct
    // language. Guard here as well so stale or odd probe data cannot create an
    // unanswerable prompt (for example two Kannada streams: 5.1 + stereo).
    if probe.is_some_and(|p| !probe::needs_language_decision(p)) {
        return StageOutcome::Unnecessary;
    }

    store.pending.push(PendingDecision {
        id,
        title: name.to_string(),
        stripped_path: stripped_path.to_path_buf(),
        multi_path: multi_path.to_path_buf(),
        recommended: recommended.map(std::string::ToString::to_string),
        candidates: build_candidates(probe, recommended),
        created_secs: now_secs(),
        last_reminded_secs: 0,
        prompt_msg_id: None,
    });
    if decisions::save(&store).is_err() {
        return StageOutcome::Failed;
    }
    StageOutcome::Pending
}

/// Whether the old stripped file may be deleted after a remux.
///
/// The `[Multi]` sidecar is never removed: when a preserved multi-track file
/// was staged with `stripped_path == multi_path`, that path *is* the multi.
#[must_use]
fn should_remove_stripped(stripped: &Path, multi: &Path, applied: &Path) -> bool {
    stripped != multi && stripped.exists() && stripped != applied
}

/// Returns true when `path` exists and its filename already carries `[language]`.
fn is_already_stripped_to(path: &Path, language: &str) -> bool {
    if !path.exists() {
        return false;
    }
    let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let tag = format!("[{}]", language.to_ascii_lowercase());
    filename.to_ascii_lowercase().contains(&tag)
}

fn apply_and_cleanup(multi: &Path, stripped: &Path, chosen: &str) -> Result<PathBuf> {
    let path = apply_language(multi, chosen)?;
    if should_remove_stripped(stripped, multi, &path) {
        let _ = std::fs::remove_file(stripped);
    }
    Ok(path)
}

/// Resolves a pending decision: applies the language and moves it to history.
pub fn resolve(id: &str, chosen: &str, auto: bool) -> Result<String> {
    let mut store = decisions::load()?;
    let Some(pos) = store.pending.iter().position(|p| p.id == id) else {
        anyhow::bail!("No pending decision with id {id}");
    };
    let entry = store.pending.remove(pos);

    let stripped_path = decisions::locate_media_path(&entry.stripped_path)
        .unwrap_or_else(|| entry.stripped_path.clone());
    let multi_path = decisions::locate_media_path(&entry.multi_path)
        .unwrap_or_else(|| entry.multi_path.clone());

    let (applied, multi_record) = if is_already_stripped_to(&stripped_path, chosen) {
        (Some(stripped_path), Some(multi_path))
    } else if multi_path.exists() {
        match apply_and_cleanup(&multi_path, &stripped_path, chosen) {
            Ok(path) => (Some(path), Some(multi_path)),
            Err(e) => {
                store.pending.insert(pos, entry);
                return Err(e);
            }
        }
    } else if stripped_path.exists() {
        (Some(stripped_path), None)
    } else {
        (None, None)
    };

    store.history.push(DecisionRecord {
        id: entry.id.clone(),
        title: entry.title.clone(),
        chosen: chosen.to_string(),
        decided_secs: now_secs(),
        auto,
        silenced: false,
        created_secs: entry.created_secs,
        multi_path: multi_record,
    });
    decisions::save(&store)?;

    let status = match applied {
        Some(_) if auto => "Auto-applied recommendation",
        Some(_) => "Applied",
        None => "Resolved (file removed)",
    };
    Ok(format!("{status} as [{chosen}]"))
}

fn ai_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(40))
        .build()
        .unwrap_or_else(|_| Client::new())
}

/// Convenience wrapper around [`recommend_language`] with a short-lived client.
#[must_use]
pub fn recommend(name: &str, probe: Option<&MediaProbe>) -> Option<String> {
    recommend_language(&ai_client(), name, probe)
}

/// Looks up a pending decision by its callback id.
#[must_use]
pub fn find_pending(id: &str) -> Option<PendingDecision> {
    decisions::load()
        .ok()?
        .pending
        .into_iter()
        .find(|p| p.id == id)
}
