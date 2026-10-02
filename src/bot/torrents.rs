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

pub fn handle_magnet(client: &Client, config: &TelegramConfig, magnet: &str) -> String {
    if let Err(e) = seedr::spawn_seedr_download(magnet, config.api_port) {
        let qb_ok = api::add_magnet(client, &config.qbittorrent_url, magnet).is_ok();
        if qb_ok {
            format!(
                "🌊 <b>領域 RYOIKI</b> • <i>qBittorrent Fallback</i>\n\
                ━━━━━━━━━━━━━━━━━━━━━━━\n\
                📥 <b>MAGNET QUEUED IN QBITTORRENT</b>\n\n\
                <i>Seedr unavailable ({e}) — forwarded to qBittorrent.</i>"
            )
        } else {
            format!("❌ <b>Failed to queue magnet in Seedr & qBittorrent:</b> {e}")
        }
    } else {
        "🌊 <b>領域 RYOIKI</b> • <i>Seedr</i>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        📥 <b>MAGNET QUEUED IN SEEDR</b>\n\n\
        Offloading to Seedr cloud.\n\
        <i>If Seedr fails, qBittorrent will automatically take over.</i>"
            .to_string()
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
                    spawn_completed_task(t.hash.clone());
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

fn spawn_completed_task(hash: String) {
    std::thread::spawn(move || {
        if let Err(e) = notify::execute("completed", &hash) {
            eprintln!("  ⚠️ Error organizing completed torrent {hash}: {e}");
        }
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
            spawn_completed_task(t.hash.clone());
        }
    } else {
        known.insert(t.hash.clone(), is_done);
        if !is_done && t.has_metadata && t.total_size > 0 {
            let text = notify::render_message("started", Some(t), host, ts_ip);
            let _ = notify::send_telegram_alert(client, &config.bot_token, &config.chat_id, &text);
        } else if is_done {
            spawn_completed_task(t.hash.clone());
        }
    }
}
