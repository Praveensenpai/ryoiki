use anyhow::Result;
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::time::Duration;

use crate::modules::torrent::api::{self, TorrentInfo};
use crate::modules::torrent::notify;
use crate::modules::torrent::report::{
    clamp_torrent_page, format_status_report, torrent_page_count,
};
use crate::modules::torrent::seedr;
use crate::notify::config::TelegramConfig;
use crate::notify::style;

use super::keyboards::torrents_keyboard;
use super::types::InlineKeyboardMarkup;

pub fn get_torrents(client: &Client, url: &str) -> Result<Vec<TorrentInfo>> {
    api::get_torrents(client, url, None)
}

/// Fetches torrents and renders one page together with its navigation keyboard.
pub fn render_torrent_report(
    client: &Client,
    url: &str,
    page: usize,
) -> Result<(String, InlineKeyboardMarkup)> {
    let torrents = api::get_torrents(client, url, None)?;
    let page = clamp_torrent_page(page, torrents.len());
    let text = format_status_report(&torrents, page);
    let keyboard = torrents_keyboard(page, torrent_page_count(torrents.len()));
    Ok((text, keyboard))
}

/// Builds a compact command view with a wordmark, headline, and detail rows.
fn event_view(title: &str, headline: &str, details: &[String]) -> String {
    let mut lines = vec![style::view(title), String::new(), headline.to_string()];
    lines.extend(details.iter().cloned());
    lines.join("\n")
}

pub fn pause_all(client: &Client, url: &str) -> Result<()> {
    api::pause_all(client, url)
}

pub fn resume_all(client: &Client, url: &str) -> Result<()> {
    api::resume_all(client, url)
}

use crate::modules::torrent::dedup::{self, Availability};
use crate::modules::torrent::scheduler::{self, SubmitOutcome};

fn check_and_restore(target: &str) -> Option<String> {
    match dedup::check_already_available(target) {
        Availability::Local { paths, title } => {
            let files_str = paths
                .iter()
                .map(|p| format!("• <code>{}</code>", p.display()))
                .collect::<Vec<_>>()
                .join("\n");
            Some(event_view(
                "DEDUPLICATION",
                &style::accent("✓", "Already available locally"),
                &[
                    format!("{}<code>{title}</code>", style::key("TITLE")),
                    format!("{}\n{files_str}", style::key("FILES")),
                    "<i>Download skipped; files exist in the Jellyfin library.</i>".to_string(),
                ],
            ))
        }
        Availability::Cloud { pairs, title } => match dedup::restore_from_cloud(&pairs) {
            Ok(restored) => {
                let files_str = restored
                    .iter()
                    .map(|p| format!("• <code>{}</code>", p.display()))
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(event_view(
                    "CLOUD RESTORE",
                    &style::accent("✓", "Restored from Google Drive"),
                    &[
                        format!("{}<code>{title}</code>", style::key("TITLE")),
                        format!("{}\n{files_str}", style::key("PATH")),
                        "<i>Copied from the Drive archive; Jellyfin refreshed.</i>".to_string(),
                    ],
                ))
            }
            Err(e) => {
                eprintln!("  ⚠️ Failed to restore from cloud: {e}");
                None
            }
        },
        Availability::NotAvailable { .. } => None,
    }
}

/// How a magnet submission should be surfaced to the user.
pub enum MagnetOutcome {
    Message(String),
    Prompt { text: String, hash: String },
}

pub fn handle_magnet(client: &Client, config: &TelegramConfig, magnet: &str) -> MagnetOutcome {
    if let Some(msg) = check_and_restore(magnet) {
        return MagnetOutcome::Message(msg);
    }

    let Some(hash) = seedr::extract_btih_hash(magnet) else {
        return MagnetOutcome::Message(fallback_to_qb(client, config, magnet, "missing info hash"));
    };

    let name = magnet_display_name(magnet).unwrap_or_else(|| hash.clone());

    // Free Seedr quota held by torrents added manually on the Seedr site; they
    // are invisible to our queue, so without this the space guard below sees a
    // full account and reroutes every magnet to qBittorrent.
    if let Some(removed) = crate::modules::torrent::seedr_reaper::reap_untracked_cloud() {
        if !removed.is_empty() {
            crate::modules::torrent::seedr_reaper::notify_reaped(&removed);
        }
    }

    if let Some(size) = seedr::extract_magnet_size(magnet) {
        if let Some(avail) = crate::modules::torrent::seedr_tasks::seedr_available_bytes() {
            if size > avail {
                // A full account is not a verdict: completed cloud items may be
                // evicted to make room. Only a still-downloading blocker sends
                // this magnet to qBittorrent instead.
                let fits = crate::modules::torrent::seedr_eviction::free_space_for(size)
                    .is_some_and(|freed| freed >= size);
                if !fits {
                    let _ = api::add_magnet(client, &config.qbittorrent_url, magnet);
                    let sz_mb = size / 1_048_576;
                    let av_mb = avail / 1_048_576;
                    return MagnetOutcome::Message(event_view(
                        "QBITTORRENT",
                        &style::accent("↓", "Magnet queued"),
                        &[format!(
                            "<i>Torrent {}.{} GB exceeds Seedr space {}.{} GB; running locally.</i>",
                            sz_mb / 1024,
                            (sz_mb % 1024) / 100,
                            av_mb / 1024,
                            (av_mb % 1024) / 100
                        )],
                    ));
                }
            }
        }
    }

    match scheduler::submit(&hash, magnet, &name, config) {
        SubmitOutcome::Started => {
            let _ = api::add_magnet(client, &config.qbittorrent_url, magnet);
            MagnetOutcome::Message(started_message())
        }
        SubmitOutcome::Duplicate => MagnetOutcome::Message(duplicate_message()),
        SubmitOutcome::Queued(pos) => {
            let _ = api::add_magnet(client, &config.qbittorrent_url, magnet);
            MagnetOutcome::Prompt {
                text: queued_prompt(pos),
                hash,
            }
        }
        SubmitOutcome::FailedStart => {
            let _ = api::add_magnet(client, &config.qbittorrent_url, magnet);
            MagnetOutcome::Message(event_view(
                "QBITTORRENT",
                &style::accent("↓", "Magnet queued"),
                &["<i>Seedr rejected the download; forwarded locally.</i>".to_string()],
            ))
        }
    }
}

fn magnet_display_name(magnet: &str) -> Option<String> {
    dedup::parse_magnet(magnet).1
}

fn started_message() -> String {
    event_view(
        "SEEDR",
        &style::accent("↓", "Download started"),
        &[
            "Caching in the Seedr cloud now.".to_string(),
            "<i>qBittorrent is downloading in parallel as a safety net.</i>".to_string(),
        ],
    )
}

fn duplicate_message() -> String {
    event_view(
        "SEEDR QUEUE",
        &style::accent("=", "Already tracked"),
        &["This magnet is active or already waiting in the queue.".to_string()],
    )
}

fn queued_prompt(pos: usize) -> String {
    event_view(
        "SEEDR QUEUE",
        &style::accent("…", &format!("Queue slot busy · position {pos}")),
        &[
            "Downloading in qBittorrent meanwhile.".to_string(),
            "<i>Seedr starts automatically when the cloud slot frees.</i>".to_string(),
            "<i>Keep it queued, or move it to the front?</i>".to_string(),
        ],
    )
}

fn fallback_to_qb(client: &Client, config: &TelegramConfig, magnet: &str, reason: &str) -> String {
    let hash = seedr::extract_btih_hash(magnet).unwrap_or_default();
    let outcome = api::ensure_torrent(client, &config.qbittorrent_url, &hash, magnet);
    if outcome == api::QbEnsure::Failed {
        return format!(
            "{} Failed to queue magnet: {reason}",
            style::accent("!", "Error")
        );
    }
    event_view(
        "QBITTORRENT",
        &style::accent("↓", "Magnet queued"),
        &[format!(
            "<i>Seedr unavailable ({reason}); forwarded locally.</i>"
        )],
    )
}

/// Classifies a `/seedr` argument.
///
/// Magnets carry an info-hash and belong in the single-slot Seedr queue (same
/// as a raw magnet message). Other targets (torrent URLs) have no hash to
/// track, so they are handed straight to `seedr-dl`.
#[must_use]
pub(crate) fn seedr_magnet_target(target: &str) -> Option<(String, String)> {
    let hash = seedr::extract_btih_hash(target)?;
    let name = dedup::parse_magnet(target)
        .1
        .unwrap_or_else(|| hash.clone());
    Some((hash, name))
}

mod seedr_cmd;
pub use seedr_cmd::{handle_seedr_cmd, handle_torrent_file};

pub fn start_torrent_monitor(config: TelegramConfig) {
    std::thread::spawn(move || {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        let mut known: HashMap<String, bool> = HashMap::new();
        let (ts_ip, host) = crate::modules::torrent::get_access_urls();
        let mut last_health = std::time::Instant::now();
        let health_interval = Duration::from_secs(30);
        let mut last_seedr_retry = std::time::Instant::now();
        let seedr_retry_interval = Duration::from_secs(3_600);

        if let Ok(items) = api::get_torrents(&client, &config.qbittorrent_url, None) {
            for t in items {
                let is_done = t.is_completed();
                known.insert(t.hash.clone(), is_done);
                if is_done {
                    spawn_completed_task(t.hash.clone(), config.clone());
                }
            }
        }

        loop {
            std::thread::sleep(Duration::from_secs(4));

            if last_health.elapsed() >= health_interval {
                last_health = std::time::Instant::now();
                crate::modules::torrent::seedr_health::record_active_health(&config);
            }

            if last_seedr_retry.elapsed() >= seedr_retry_interval {
                last_seedr_retry = std::time::Instant::now();
                crate::modules::torrent::seedr_retry::retry_candidates(&client, &config);
            }

            let Ok(torrents) = api::get_torrents(&client, &config.qbittorrent_url, None) else {
                continue;
            };

            for t in &torrents {
                check_torrent_event(&client, &config, (&host, &ts_ip), &mut known, t);
            }
        }
    });
}

fn spawn_completed_task(hash: String, config: TelegramConfig) {
    std::thread::spawn(move || {
        if let Err(e) = notify::execute("completed", &hash) {
            eprintln!("  ⚠️ Error organizing completed torrent {hash}: {e}");
        }
        // qBittorrent won the race: cancel any Seedr copy and promote the next.
        scheduler::handle_qb_completion(&hash, &config);
    });
}

fn check_torrent_event(
    client: &Client,
    config: &TelegramConfig,
    (host, ts_ip): (&str, &str),
    known: &mut HashMap<String, bool>,
    t: &TorrentInfo,
) {
    let is_done = t.is_completed();
    if let Some(was_done) = known.get_mut(&t.hash) {
        if !*was_done && is_done {
            *was_done = true;
            spawn_completed_task(t.hash.clone(), config.clone());
        }
    } else {
        known.insert(t.hash.clone(), is_done);
        if !is_done && t.has_metadata && t.total_size > 0 {
            let text = notify::render_message("started", Some(t), host, ts_ip);
            let _ = notify::send_telegram_alert(client, &config.bot_token, &config.chat_id, &text);
        } else if is_done {
            spawn_completed_task(t.hash.clone(), config.clone());
        }
    }
}
