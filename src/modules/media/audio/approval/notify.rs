//! Notification dispatch, expiration handling, and presentation rendering.

use anyhow::Result;
use reqwest::blocking::Client;

use super::super::decisions::{self, days_left, now_secs, DecisionRecord, PendingDecision};
use super::super::super::probe::MediaProbe;
use super::{recommend_language, resolve};
use crate::bot::keyboards::language_keyboard;
use crate::notify::client::{escape_html, format_card, send_alert};
use crate::notify::config::TelegramConfig;

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

fn prune_stale_pending(store: &mut decisions::DecisionStore, now: u64) {
    let mut to_drop = Vec::new();
    for (i, entry) in store.pending.iter_mut().enumerate() {
        if let Some(sp) = decisions::locate_media_path(&entry.stripped_path) {
            entry.stripped_path = sp;
        }
        if let Some(mp) = decisions::locate_media_path(&entry.multi_path) {
            entry.multi_path = mp;
        }
        if !entry.stripped_path.exists() && !entry.multi_path.exists() {
            to_drop.push(i);
        }
    }
    for i in to_drop.into_iter().rev() {
        let entry = store.pending.remove(i);
        store.history.push(DecisionRecord {
            id: entry.id,
            title: entry.title,
            chosen: "file removed".to_string(),
            decided_secs: now,
            auto: true,
            silenced: true,
            created_secs: entry.created_secs,
            multi_path: None,
        });
    }
}

fn dispatch_prompt(
    client: &Client,
    config: &TelegramConfig,
    entry: &mut PendingDecision,
    now: u64,
) -> bool {
    if entry.recommended.is_none() {
        let probe = MediaProbe {
            audio_languages: entry.candidates.clone(),
            ..MediaProbe::default()
        };
        entry.recommended = recommend_language(client, &entry.title, Some(&probe));
    }

    if entry.last_reminded_secs > 0
        && now.saturating_sub(entry.last_reminded_secs) < decisions::SECS_PER_DAY
    {
        return false;
    }

    if let Some(old_id) = entry.prompt_msg_id {
        let _ = crate::bot::client::delete_message(
            client,
            &config.bot_token,
            &config.chat_id,
            old_id,
        );
    }

    let card = render_prompt(entry);
    let keyboard = language_keyboard(
        &entry.id,
        &entry.candidates,
        entry.recommended.as_deref(),
    );
    let msg_id = crate::bot::client::reply_with_keyboard_id(
        client,
        &config.bot_token,
        &config.chat_id,
        &card,
        &keyboard,
    )
    .ok();
    entry.last_reminded_secs = now;
    entry.prompt_msg_id = msg_id;
    true
}

/// Sends one Telegram prompt per pending decision, and applies overdue ones.
pub fn notify_pending(config: &TelegramConfig) -> Result<usize> {
    let _ = super::super::cleanup::cleanup_stale_multis();
    let _ = expire_overdue(config);
    let mut store = decisions::load()?;
    let client = notify_client();
    let now = now_secs();

    prune_stale_pending(&mut store, now);

    let mut sent = 0usize;
    for entry in &mut store.pending {
        if dispatch_prompt(&client, config, entry, now) {
            sent += 1;
        }
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
        "{}\n\n<b>WHICH TRACK IS THE ORIGINAL?</b>\n\n{}\n{}\n{}\n\n<i>No reply within {remaining} day(s); AI recommends is applied and the [Multi] copy is kept.</i>",
        crate::notify::style::alert("DubStrip"),
        crate::notify::style::row("TITLE", &format!("<code>{title_esc}</code>")),
        crate::notify::style::row("AI RECOMMENDS", &format!("<b>{rec_esc}</b>")),
        crate::notify::style::row("TRACKS", &format!("<code>{langs_esc}</code>")),
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
