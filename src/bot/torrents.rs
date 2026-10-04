use anyhow::Result;
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::time::Duration;

use crate::modules::torrent::api::{self, TorrentInfo};
use crate::modules::torrent::notify;
use crate::modules::torrent::report::format_status_report;
use crate::modules::torrent::seedr;
use crate::notify::config::TelegramConfig;

pub fn get_torrents(client: &Client, url: &str) -> Result<Vec<TorrentInfo>> {
    api::get_torrents(client, url, None)
}

pub fn render_torrent_report(client: &Client, url: &str) -> Result<String> {
    let torrents = api::get_torrents(client, url, None)?;
    Ok(format_status_report(&torrents))
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
            Some(format!(
                "🌊 <b>領域 RYOIKI</b> • <i>Library Deduplication</i>\n\
                ━━━━━━━━━━━━━━━━━━━━━━━\n\
                ✨ <b>ALREADY AVAILABLE LOCALLY</b>\n\n\
                🎬 <b>Title:</b> <code>{title}</code>\n\
                📁 <b>Files:</b>\n{files_str}\n\n\
                <i>Skipping download — file(s) exist in your Jellyfin media library.</i>"
            ))
        }
        Availability::Cloud { pairs, title } => match dedup::restore_from_cloud(&pairs) {
            Ok(restored) => {
                let files_str = restored
                    .iter()
                    .map(|p| format!("• <code>{}</code>", p.display()))
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(format!(
                    "🌊 <b>領域 RYOIKI</b> • <i>Cloud Archive Restore</i>\n\
                    ━━━━━━━━━━━━━━━━━━━━━━━\n\
                    ☁️ <b>RESTORED FROM GOOGLE DRIVE</b>\n\n\
                    🎬 <b>Title:</b> <code>{title}</code>\n\
                    📍 <b>Restored To:</b>\n{files_str}\n\n\
                    <i>Copied directly from Drive archive & refreshed Jellyfin.</i>"
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

    match scheduler::submit(&hash, magnet, &name, config) {
        SubmitOutcome::Started => MagnetOutcome::Message(started_message()),
        SubmitOutcome::Duplicate => MagnetOutcome::Message(duplicate_message()),
        SubmitOutcome::Queued(pos) => {
            // Queued in Seedr, but download in qBittorrent in parallel right away.
            let _ = api::add_magnet(client, &config.qbittorrent_url, magnet);
            MagnetOutcome::Prompt {
                text: queued_prompt(pos),
                hash,
            }
        }
    }
}

fn magnet_display_name(magnet: &str) -> Option<String> {
    dedup::parse_magnet(magnet).1
}

fn started_message() -> String {
    "🌊 <b>領域 RYOIKI</b> • <i>Seedr</i>\n\
    ━━━━━━━━━━━━━━━━━━━━━━━\n\
    📥 <b>SEEDR DOWNLOAD STARTED</b>\n\n\
    ☁️ Caching in Seedr cloud now.\n\
    <i>qBittorrent picks it up as a fallback if Seedr fails.</i>"
        .to_string()
}

fn duplicate_message() -> String {
    "🌊 <b>領域 RYOIKI</b> • <i>Seedr Queue</i>\n\
    ━━━━━━━━━━━━━━━━━━━━━━━\n\
    ♻️ <b>ALREADY TRACKED</b>\n\n\
    This magnet is already active or waiting in the Seedr queue."
        .to_string()
}

fn queued_prompt(pos: usize) -> String {
    format!(
        "🌊 <b>領域 RYOIKI</b> • <i>Seedr Queue</i>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        ⏳ <b>SEEDR SLOT BUSY — QUEUED AT #{pos}</b>\n\n\
        📥 Downloading in qBittorrent meanwhile.\n\
        <i>Seedr starts automatically when the current cloud slot frees.</i>\n\n\
        Keep it queued, or move it to the front?"
    )
}

fn fallback_to_qb(client: &Client, config: &TelegramConfig, magnet: &str, reason: &str) -> String {
    if api::add_magnet(client, &config.qbittorrent_url, magnet).is_ok() {
        format!(
            "🌊 <b>領域 RYOIKI</b> • <i>qBittorrent</i>\n\
            ━━━━━━━━━━━━━━━━━━━━━━━\n\
            📥 <b>MAGNET QUEUED IN QBITTORRENT</b>\n\n\
            <i>Seedr unavailable ({reason}) — forwarded to qBittorrent.</i>"
        )
    } else {
        format!("❌ <b>Failed to queue magnet in Seedr & qBittorrent:</b> {reason}")
    }
}

pub fn handle_seedr_cmd(target: &str, api_port: u16) -> String {
    if target.is_empty() || target == "status" {
        let tasks = seedr::get_active_seedr_tasks();
        let sec = seedr::format_seedr_tasks_section(&tasks);
        if sec.is_empty() {
            "🌱 <b>Seedr Cloud:</b> No active downloads.\nTip: <code>/seedr &lt;magnet&gt;</code>"
                .to_string()
        } else {
            format!("🌊 <b>領域 RYOIKI • Seedr</b>\n━━━━━━━━━━━━━━━━━━━━━━━\n{sec}")
        }
    } else {
        if let Some(msg) = check_and_restore(target) {
            return msg;
        }

        match seedr::spawn_seedr_download(target, api_port) {
            Ok(()) => "🌊 <b>領域 RYOIKI</b> • <i>Seedr</i>\n\
                ━━━━━━━━━━━━━━━━━━━━━━━\n\
                📥 <b>SEEDR DOWNLOAD QUEUED</b>\n\n\
                Seedr.cc cloud caching initiated!"
                .to_string(),
            Err(e) => format!("❌ <b>Failed to spawn Seedr download:</b>\n<code>{e}</code>"),
        }
    }
}

pub fn handle_torrent_file(
    client: &Client,
    config: &TelegramConfig,
    fname: &str,
    bytes: Vec<u8>,
) -> Result<String> {
    if let Some(msg) = check_and_restore(fname) {
        return Ok(msg);
    }

    api::add_torrent_file(client, &config.qbittorrent_url, fname, bytes)?;
    Ok(format!(
        "🌊 <b>領域 RYOIKI</b> • <i>qBittorrent</i>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        📥 <b>TORRENT FILE QUEUED</b>\n\n\
        <code>{fname}</code> added to downloads!"
    ))
}

pub fn start_torrent_monitor(config: TelegramConfig) {
    std::thread::spawn(move || {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        let mut known: HashMap<String, bool> = HashMap::new();
        let (ts_ip, host) = crate::modules::torrent::get_access_urls();

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
