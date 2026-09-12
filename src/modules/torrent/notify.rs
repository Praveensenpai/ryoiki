use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::thread::sleep;
use std::time::Duration;

use super::api::{self, TorrentInfo};
use super::telegram::TelegramConfig;
use crate::modules::media::OrganizeResult;

pub fn execute(event: &str, hash: &str) -> Result<()> {
    let config = TelegramConfig::load()?;
    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .context("Failed to build HTTP client")?;

    let base_url = &config.qbittorrent_url;
    let torrent = resolve_torrent_data(&client, base_url, event, hash);

    let (organized, qb_cleared) = if event == "completed" {
        process_completed_media(
            &client,
            base_url,
            torrent.as_ref(),
            config.gemini_api_key.as_deref(),
        )
    } else {
        (None, false)
    };

    let (ts_ip, host) = super::get_access_urls();
    let text = match organized.as_ref() {
        Some(org) => render_organized_message(org, &host, &ts_ip, qb_cleared),
        None => render_message(event, torrent.as_ref(), &host, &ts_ip),
    };

    send_telegram_alert(&client, &config.bot_token, &config.chat_id, &text)?;
    Ok(())
}

fn process_completed_media(
    client: &Client,
    base_url: &str,
    torrent: Option<&TorrentInfo>,
    gemini_key: Option<&str>,
) -> (Option<OrganizeResult>, bool) {
    let Some(t) = torrent else {
        return (None, false);
    };

    let organized =
        crate::modules::media::organizer::organize_completed_torrent(client, t, gemini_key)
            .ok()
            .flatten();

    let cleared = if organized.is_some() || t.progress >= 1.0 {
        api::delete_torrent(client, base_url, &t.hash, false).is_ok()
    } else {
        false
    };

    (organized, cleared)
}

fn resolve_torrent_data(
    client: &Client,
    url: &str,
    event: &str,
    hash: &str,
) -> Option<TorrentInfo> {
    if hash.is_empty() {
        return None;
    }

    let max_attempts = if event == "started" { 30 } else { 3 };
    for _ in 0..max_attempts {
        if let Ok(torrents) = api::get_torrents(client, url, Some(hash)) {
            if let Some(t) = torrents.into_iter().next() {
                if event != "started" || (t.has_metadata && t.total_size > 0) {
                    return Some(t);
                }
            }
        }
        if event != "started" {
            break;
        }
        sleep(Duration::from_secs(2));
    }
    None
}

pub fn format_size(bytes: u64) -> String {
    let mut b = bytes;
    let mut rem = 0;
    let mut unit = "B";
    for u in ["KB", "MB", "GB", "TB"] {
        if b < 1024 {
            break;
        }
        rem = (b % 1024) * 100 / 1024;
        b /= 1024;
        unit = u;
    }
    if unit == "B" {
        format!("{b} B")
    } else {
        format!("{b}.{rem:02} {unit}")
    }
}
use crate::notify::client::escape_html;

pub(crate) fn render_message(
    event: &str,
    torrent: Option<&TorrentInfo>,
    host: &str,
    ts_ip: &str,
) -> String {
    let badge = if event == "started" {
        "📥 <b>DOWNLOAD INITIATED</b>"
    } else if event == "completed" {
        "✨ <b>DOWNLOAD COMPLETED</b>"
    } else {
        "ℹ️ <b>TORRENT EVENT</b>"
    };

    let (name, size_str, category) = match torrent {
        Some(t) => (
            escape_html(&t.name),
            format_size(t.total_size),
            if t.category.is_empty() {
                "Default".to_string()
            } else {
                escape_html(&t.category)
            },
        ),
        None => (
            "Unknown Torrent".to_string(),
            "Unknown".to_string(),
            "Default".to_string(),
        ),
    };

    let webui_url = if ts_ip.is_empty() {
        "http://localhost:6881".to_string()
    } else {
        format!("http://{ts_ip}:6881")
    };

    format!(
        "🌊 <b>領域 RYOIKI</b> • <i>qBittorrent</i>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        {badge}\n\n\
        📦 <b>File:</b> <code>{name}</code>\n\
        📊 <b>Size:</b> <code>{size_str}</code>\n\
        🏷 <b>Category:</b> <code>{category}</code>\n\
        🖥 <b>Host:</b> <code>{host}</code> ({ts_ip})\n\n\
        🌐 <a href=\"{webui_url}\">Open WebUI</a> • <i>Tailscale</i>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━"
    )
}

pub(crate) fn render_organized_message(
    org: &OrganizeResult,
    host: &str,
    ts_ip: &str,
    qb_cleared: bool,
) -> String {
    let clean_name = escape_html(&org.media_info.clean_name);
    let dest = escape_html(&org.dest_path.display().to_string());
    let qb_status = if qb_cleared {
        "History cleared"
    } else {
        "Kept in client"
    };

    format!(
        "🌊 <b>領域 RYOIKI</b> • <i>Media Organizer</i>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        ✨ <b>MEDIA ORGANIZED & MOVED</b>\n\n\
        🎬 <b>Title:</b> <code>{clean_name}</code>\n\
        📂 <b>Type:</b> {}\n\
        ⚙️ <b>Engine:</b> {}\n\
        📍 <b>Destination:</b> <code>{dest}</code>\n\
        🗑 <b>qBittorrent:</b> {qb_status}\n\
        🖥 <b>Host:</b> <code>{host}</code> ({ts_ip})\n\
        ━━━━━━━━━━━━━━━━━━━━━━━",
        org.media_info.media_type, org.media_info.engine,
    )
}

pub(crate) fn send_telegram_alert(
    client: &Client,
    token: &str,
    chat_id: &str,
    text: &str,
) -> Result<()> {
    crate::notify::client::send_telegram_alert(client, token, chat_id, text)
}
