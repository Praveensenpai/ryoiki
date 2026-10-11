//! Original-language approval: AI recommendation, Telegram prompt, and the
//! ffmpeg remux that reduces the `[Multi]` sidecar to one chosen track.

use anyhow::Result;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::path::Path;

use super::super::probe::MediaProbe;
use super::decisions::{self, days_left, find_history, now_secs, DecisionRecord, PendingDecision};
use super::remux::apply_language;
use crate::bot::keyboards::language_keyboard;
use crate::notify::client::{escape_html, format_card, send_alert};
use crate::notify::config::TelegramConfig;

const MAX_CANDIDATES: usize = 6;

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
        return StageOutcome::Duplicate(format!(
            "already selected as [{}] — skipping duplicate",
            rec.chosen
        ));
    }

    if let Some(existing) = store.pending.iter_mut().find(|p| p.id == id) {
        existing.stripped_path = stripped_path.to_path_buf();
        existing.multi_path = multi_path.to_path_buf();
        existing.title = name.to_string();
        let _ = decisions::save(&store);
        return StageOutcome::Pending;
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

/// Resolves a pending decision: applies the language and moves it to history.
pub fn resolve(id: &str, chosen: &str, auto: bool) -> Result<String> {
    let mut store = decisions::load()?;
    let Some(pos) = store.pending.iter().position(|p| p.id == id) else {
        anyhow::bail!("No pending decision with id {id}");
    };
    let entry = store.pending.remove(pos);

    let applied = match apply_language(&entry.multi_path, chosen) {
        Ok(path) => path,
        Err(e) => {
            store.pending.insert(pos, entry);
            return Err(e);
        }
    };
    if entry.stripped_path.exists() && entry.stripped_path != applied {
        let _ = std::fs::remove_file(&entry.stripped_path);
    }

    store.history.push(DecisionRecord {
        id: entry.id.clone(),
        title: entry.title.clone(),
        chosen: chosen.to_string(),
        decided_secs: now_secs(),
        auto,
    });
    decisions::save(&store)?;

    Ok(format!(
        "{} as [{}]",
        if auto {
            "Auto-applied recommendation"
        } else {
            "Applied"
        },
        chosen
    ))
}

/// Applies the AI recommendation for every pending entry past the grace window.
///
/// Returns the number of entries auto-resolved.
pub fn expire_overdue(config: &TelegramConfig) -> usize {
    let Ok(store) = decisions::load() else {
        return 0;
    };
    let mut applied = 0usize;
    for entry in store.pending.clone() {
        if days_left(&entry) > 0 {
            continue;
        }
        let Some(chosen) = entry.recommended.clone() else {
            continue;
        };
        if let Ok(msg) = resolve(&entry.id, &chosen, true) {
            applied += 1;
            let card = format_card(
                "DubStrip",
                "⌛ <b>GRACE WINDOW EXPIRED</b>",
                &[
                    ("🎬 Title:", entry.title.as_str()),
                    ("🌐 Language:", chosen.as_str()),
                    ("ℹ Result:", msg.as_str()),
                ],
            );
            let _ = send_alert(&config.bot_token, &config.chat_id, &card);
        }
    }
    applied
}

/// Sends one Telegram prompt per pending decision, and applies overdue ones.
pub fn notify_pending(config: &TelegramConfig) -> Result<usize> {
    let _ = expire_overdue(config);
    let mut store = decisions::load()?;
    let client = notify_client();
    let mut sent = 0usize;

    let now = now_secs();
    for idx in 0..store.pending.len() {
        if store.pending[idx].recommended.is_none() {
            let candidates = store.pending[idx].candidates.clone();
            let title = store.pending[idx].title.clone();
            let probe = MediaProbe {
                audio_languages: candidates,
                ..MediaProbe::default()
            };
            store.pending[idx].recommended = recommend_language(&client, &title, Some(&probe));
        }

        // Re-remind daily: only skip when a prompt went out within the last day.
        let last = store.pending[idx].last_reminded_secs;
        if last > 0 && now.saturating_sub(last) < decisions::SECS_PER_DAY {
            continue;
        }

        let card = render_prompt(&store.pending[idx]);
        let keyboard = language_keyboard(
            &store.pending[idx].id,
            &store.pending[idx].candidates,
            store.pending[idx].recommended.as_deref(),
        );
        let msg_id = crate::bot::client::reply_with_keyboard_id(
            &client,
            &config.bot_token,
            &config.chat_id,
            &card,
            &keyboard,
        )
        .ok();
        store.pending[idx].last_reminded_secs = now_secs();
        store.pending[idx].prompt_msg_id = msg_id;
        sent += 1;
    }
    decisions::save(&store)?;
    Ok(sent)
}

fn notify_client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| Client::new())
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

/// Renders the interactive prompt card for one pending decision.
#[must_use]
pub fn render_prompt(entry: &PendingDecision) -> String {
    let rec = entry.recommended.as_deref().unwrap_or("unknown");
    let remaining = days_left(entry);
    let title_esc = escape_html(&entry.title);
    let rec_esc = escape_html(rec);
    let langs = entry.candidates.join(", ");
    let langs_esc = escape_html(&langs);
    format!(
        "🌊 <b>領域 RYOIKI</b> • <i>DubStrip</i>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        ❓ <b>WHICH TRACK IS THE ORIGINAL?</b>\n\n\
        🎬 Title: <code>{title_esc}</code>\n\
        🤖 AI recommends: <b>{rec_esc}</b>\n\
        🎚 Tracks: <code>{langs_esc}</code>\n\n\
        <i>No reply within {remaining} day(s) — the recommendation is applied and the [Multi] copy is kept.</i>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━"
    )
}

/// Plain-text listing of pending decisions for the CLI and bot.
#[must_use]
pub fn list_pending_text() -> String {
    let Ok(store) = decisions::load() else {
        return "No pending audio decisions.".to_string();
    };
    if store.pending.is_empty() {
        return "No pending audio decisions.".to_string();
    }
    let mut lines = vec![format!(
        "{} pending audio decision(s):",
        store.pending.len()
    )];
    for e in &store.pending {
        let rec = e.recommended.as_deref().unwrap_or("unknown");
        lines.push(format!(
            "  • {} — AI: {} — {} day(s) left",
            e.title,
            rec,
            days_left(e)
        ));
    }
    lines.join("\n")
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

#[cfg(test)]
mod tests {
    use super::super::decisions::DecisionStore;
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_build_candidates_prefers_recommendation() {
        let probe = MediaProbe {
            audio_languages: vec!["Tamil".to_string(), "Telugu".to_string()],
            ..Default::default()
        };
        let c = build_candidates(Some(&probe), Some("Telugu"));
        assert_eq!(c[0], "Telugu");
        assert!(c.contains(&"Tamil".to_string()));
    }

    #[test]
    fn test_build_candidates_fallback() {
        let c = build_candidates(None, None);
        assert_eq!(c, vec!["Original".to_string()]);
    }

    #[test]
    fn test_render_prompt_marks_recommendation() {
        let entry = PendingDecision {
            id: "abc".to_string(),
            title: "Sample (2024) [Multi].mkv".to_string(),
            stripped_path: PathBuf::from("/a.mkv"),
            multi_path: PathBuf::from("/b.mkv"),
            recommended: Some("Telugu".to_string()),
            candidates: vec!["Telugu".to_string(), "Tamil".to_string()],
            created_secs: now_secs(),
            last_reminded_secs: 0,
            prompt_msg_id: None,
        };
        let card = render_prompt(&entry);
        assert!(card.contains("Telugu"));
        assert!(card.contains("AI recommends"));
    }

    #[test]
    fn test_store_roundtrip_serialization() {
        let store = DecisionStore::default();
        let json = serde_json::to_string(&store).unwrap_or_default();
        assert!(serde_json::from_str::<DecisionStore>(&json).is_ok());
    }
}
