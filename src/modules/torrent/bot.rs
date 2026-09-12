use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::CString;
use std::mem::MaybeUninit;
use std::time::Duration;

use super::api::{self, TorrentInfo};
use super::notify::{self, format_size};
use super::telegram::TelegramConfig;

#[derive(Debug, Deserialize)]
struct TelegramResponse<T> {
    result: Option<T>,
}

#[derive(Debug, Deserialize)]
struct Update {
    update_id: i64,
    message: Option<Message>,
}

#[derive(Debug, Deserialize)]
struct Message {
    from: Option<User>,
    text: Option<String>,
    document: Option<Document>,
}

#[derive(Debug, Deserialize)]
struct User {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct Document {
    file_id: String,
    file_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FileResult {
    file_path: Option<String>,
}

pub fn run_bot() -> Result<()> {
    let config = TelegramConfig::load()?;
    let client = Client::builder()
        .timeout(Duration::from_secs(40))
        .build()
        .context("Failed to initialize HTTP client for bot")?;

    println!("  ⚡ Ryoiki Pure-Rust Telegram Bot active (Listening for commands)...");
    crate::notify::server::spawn_background_server(config.clone(), config.api_port);
    start_torrent_monitor(config.clone());
    let mut offset: i64 = 0;

    loop {
        let Ok(updates) = fetch_updates(&client, &config.bot_token, offset) else {
            std::thread::sleep(Duration::from_secs(3));
            continue;
        };

        for u in updates {
            offset = u.update_id + 1;
            if let Some(msg) = u.message {
                handle_incoming_message(&client, &config, msg);
            }
        }
    }
}

fn fetch_updates(client: &Client, token: &str, offset: i64) -> Result<Vec<Update>> {
    let url = format!("https://api.telegram.org/bot{token}/getUpdates");
    let resp: TelegramResponse<Vec<Update>> = client
        .get(&url)
        .query(&[
            ("offset", offset.to_string()),
            ("timeout", "25".to_string()),
        ])
        .send()?
        .json()?;

    Ok(resp.result.unwrap_or_default())
}

fn handle_incoming_message(client: &Client, config: &TelegramConfig, msg: Message) {
    let Ok(auth_id) = config.chat_id.parse::<i64>() else {
        return;
    };

    if msg.from.is_none_or(|u| u.id != auth_id) {
        return;
    }

    if let Some(doc) = msg.document {
        let _ = handle_document(client, config, doc);
        return;
    }

    if let Some(text) = msg.text {
        let trimmed = text.trim();
        if trimmed.starts_with("magnet:?xt=urn:") {
            let _ = handle_magnet(client, config, trimmed);
        } else if trimmed.starts_with('/') {
            let cmd = trimmed.split_whitespace().next().unwrap_or("");
            let _ = handle_command(client, config, cmd);
        }
    }
}

fn handle_magnet(client: &Client, config: &TelegramConfig, magnet: &str) -> Result<()> {
    match api::add_magnet(client, &config.qbittorrent_url, magnet) {
        Ok(()) => {
            let msg = "🌊 <b>領域 RYOIKI</b> • <i>qBittorrent</i>\n━━━━━━━━━━━━━━━━━━━━━━━\n📥 <b>MAGNET LINK QUEUED</b>\n\nqBittorrent is fetching torrent metadata...";
            reply(client, config, msg)?;
        }
        Err(e) => {
            let err_msg = format!("❌ <b>Failed to add magnet:</b>\n<code>{e}</code>");
            reply(client, config, &err_msg)?;
        }
    }
    Ok(())
}

fn handle_document(client: &Client, config: &TelegramConfig, doc: Document) -> Result<()> {
    let fname = doc
        .file_name
        .unwrap_or_else(|| "download.torrent".to_string());
    if !fname.ends_with(".torrent") {
        return Ok(());
    }

    let file_url = format!(
        "https://api.telegram.org/bot{}/getFile?file_id={}",
        config.bot_token, doc.file_id
    );
    let f_res: TelegramResponse<FileResult> = client.get(&file_url).send()?.json()?;
    let Some(path) = f_res.result.and_then(|r| r.file_path) else {
        return Ok(());
    };

    let dl_url = format!(
        "https://api.telegram.org/file/bot{}/{path}",
        config.bot_token
    );
    let bytes = client.get(&dl_url).send()?.bytes()?.to_vec();

    api::add_torrent_file(client, &config.qbittorrent_url, &fname, bytes)?;
    let msg = format!("🌊 <b>領域 RYOIKI</b> • <i>qBittorrent</i>\n━━━━━━━━━━━━━━━━━━━━━━━\n📥 <b>TORRENT FILE QUEUED</b>\n\n<code>{fname}</code> added to downloads!");
    reply(client, config, &msg)?;
    Ok(())
}

fn handle_command(client: &Client, config: &TelegramConfig, cmd: &str) -> Result<()> {
    match cmd {
        "/status" => {
            let torrents = api::get_torrents(client, &config.qbittorrent_url, None)?;
            let text = format_status_report(&torrents);
            reply(client, config, &text)?;
        }
        "/disk" => {
            let text = format_disk_report();
            reply(client, config, &text)?;
        }
        "/pause" => {
            api::pause_all(client, &config.qbittorrent_url)?;
            reply(client, config, "⏸ <b>All torrents paused</b>")?;
        }
        "/resume" => {
            api::resume_all(client, &config.qbittorrent_url)?;
            reply(client, config, "▶️ <b>All torrents resumed</b>")?;
        }
        "/organize" | "/organise" => {
            let text = handle_bot_organize(config)?;
            reply(client, config, &text)?;
        }
        "/help" | "/start" => {
            let help_text = "🌊 <b>領域 RYOIKI • Command Center</b>\n━━━━━━━━━━━━━━━━━━━━━━━\n\
                🧲 <i>Paste any magnet link to start download</i>\n\
                📎 <i>Upload a .torrent file to start download</i>\n\n\
                📊 /status — Live progress, speeds, & ETAs\n\
                💾 /disk — Free NVMe/SSD storage space\n\
                🎬 /organize — Classify & move completed media\n\
                ⏸ /pause — Pause all active downloads\n\
                ▶️ /resume — Resume all paused downloads\n\
                ❓ /help — Show this command list\n━━━━━━━━━━━━━━━━━━━━━━━";
            reply(client, config, help_text)?;
        }
        _ => {}
    }
    Ok(())
}

fn handle_bot_organize(config: &TelegramConfig) -> Result<String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let target = std::path::Path::new(&home).join("torrents");
    let http_client = Client::builder().timeout(Duration::from_secs(30)).build()?;
    let api_key = config.gemini_api_key.as_deref();

    let results =
        crate::modules::media::organizer::organize_path(&target, &http_client, api_key, false)?;

    let cleared = crate::modules::media::organizer::cleanup_matching_torrents(
        &http_client,
        &config.qbittorrent_url,
        &results,
    );

    if results.is_empty() {
        if cleared > 0 {
            return Ok(format!(
                "🌊 <b>領域 RYOIKI • Media Organizer</b>\n\
                ━━━━━━━━━━━━━━━━━━━━━━━\n\
                🗑 <b>Cleaned {cleared} torrent(s) from qBittorrent history.</b>"
            ));
        }
        return Ok("🌊 <b>領域 RYOIKI • Media Organizer</b>\n━━━━━━━━━━━━━━━━━━━━━━━\nNo new video files found to organize in ~/torrents.".to_string());
    }

    let mut lines = vec![
        "🌊 <b>領域 RYOIKI • Media Organizer</b>".to_string(),
        "━━━━━━━━━━━━━━━━━━━━━━━".to_string(),
        format!(
            "✨ <b>Organized {} item(s) into Jellyfin</b>\n",
            results.len()
        ),
    ];

    for res in results.iter().take(5) {
        let clean = crate::notify::client::escape_html(&res.media_info.clean_name);
        lines.push(format!(
            "🎬 <code>{clean}</code> ({})",
            res.media_info.engine
        ));
    }

    if results.len() > 5 {
        lines.push(format!("<i>...and {} more items</i>", results.len() - 5));
    }

    if cleared > 0 {
        lines.push(format!(
            "\n🗑 <i>Removed {cleared} torrent(s) from qBittorrent</i>"
        ));
    }

    lines.push("━━━━━━━━━━━━━━━━━━━━━━━".to_string());
    Ok(lines.join("\n"))
}

fn format_status_report(torrents: &[TorrentInfo]) -> String {
    if torrents.is_empty() {
        return "🌊 <b>領域 RYOIKI • Torrents</b>\n━━━━━━━━━━━━━━━━━━━━━━━\nNo active or completed torrents found.".to_string();
    }

    let mut lines = vec!["🌊 <b>領域 RYOIKI • Torrents</b>\n━━━━━━━━━━━━━━━━━━━━━━━".to_string()];
    for t in torrents.iter().take(5) {
        let pct = (t.progress * 100.0).clamp(0.0, 100.0);
        let blocks = format!("{:.0}", pct / 10.0)
            .parse::<usize>()
            .unwrap_or(0)
            .min(10);
        let bar = format!(
            "[{}{}] {pct:.1}%",
            "█".repeat(blocks),
            "░".repeat(10 - blocks)
        );
        let speed = format!(
            "DL: {}/s | UL: {}/s",
            format_size(t.dlspeed),
            format_size(t.upspeed)
        );
        lines.push(format!(
            "📦 <b>{}</b>\n<code>{bar}</code> • <b>{}</b>\nSize: {} | {speed} | ETA: {}\n",
            t.name,
            t.state,
            format_size(t.total_size),
            format_eta(t.eta)
        ));
    }

    if torrents.len() > 5 {
        lines.push(format!(
            "<i>...and {} more torrents</i>",
            torrents.len() - 5
        ));
    }
    lines.push("━━━━━━━━━━━━━━━━━━━━━━━".to_string());
    lines.join("\n")
}

fn format_eta(eta: i64) -> String {
    if eta <= 0 || eta >= 8_640_000 {
        "∞".to_string()
    } else if eta < 60 {
        format!("{eta}s")
    } else if eta < 3600 {
        format!("{}m {}s", eta / 60, eta % 60)
    } else {
        format!("{}h {}m", eta / 3600, (eta % 3600) / 60)
    }
}

fn format_disk_report() -> String {
    let (total, used, free) = get_disk_info("/home/neko/torrents").unwrap_or((0, 0, 0));
    let pct = (used * 100).checked_div(total).unwrap_or(0);

    format!(
        "💾 <b>Disk Usage • mochi</b>\n━━━━━━━━━━━━━━━━━━━━━━━\n<b>Total:</b> {}\n<b>Used:</b>  {} ({pct}%)\n<b>Free:</b>  {}\n━━━━━━━━━━━━━━━━━━━━━━━",
        format_size(total),
        format_size(used),
        format_size(free)
    )
}

fn get_disk_info(path: &str) -> Option<(u64, u64, u64)> {
    let c_path = CString::new(path).ok()?;
    let mut stat = MaybeUninit::<libc::statvfs>::uninit();
    let res = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if res == 0 {
        let s = unsafe { stat.assume_init() };
        let total = s.f_blocks * s.f_frsize;
        let free = s.f_bavail * s.f_frsize;
        let used = total.saturating_sub(free);
        Some((total, used, free))
    } else {
        None
    }
}

fn reply(client: &Client, config: &TelegramConfig, text: &str) -> Result<()> {
    crate::notify::client::send_telegram_alert(client, &config.bot_token, &config.chat_id, text)
}

fn start_torrent_monitor(config: TelegramConfig) {
    std::thread::spawn(move || {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        let mut known: HashMap<String, bool> = HashMap::new();
        let (ts_ip, host) = super::get_access_urls();

        if let Ok(items) = api::get_torrents(&client, &config.qbittorrent_url, None) {
            for t in items {
                let is_done = is_completed(&t);
                known.insert(t.hash, is_done);
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

fn is_completed(t: &TorrentInfo) -> bool {
    t.progress >= 1.0 || t.state == "pausedUP" || t.state == "stalledUP" || t.state == "uploading"
}

fn check_torrent_event(
    client: &Client,
    config: &TelegramConfig,
    (host, ts_ip): (&str, &str),
    known: &mut HashMap<String, bool>,
    t: &TorrentInfo,
) {
    let is_done = is_completed(t);
    if let Some(was_done) = known.get_mut(&t.hash) {
        if !*was_done && is_done {
            *was_done = true;
            let text = notify::render_message("completed", Some(t), host, ts_ip);
            let _ = notify::send_telegram_alert(client, &config.bot_token, &config.chat_id, &text);
        }
    } else {
        known.insert(t.hash.clone(), is_done);
        if !is_done && t.has_metadata && t.total_size > 0 {
            let text = notify::render_message("started", Some(t), host, ts_ip);
            let _ = notify::send_telegram_alert(client, &config.bot_token, &config.chat_id, &text);
        }
    }
}
