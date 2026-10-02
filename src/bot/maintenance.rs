use anyhow::Result;
use reqwest::blocking::Client;
use std::time::Duration;

use crate::modules::torrent::api::{self, TorrentInfo};
use crate::notify::client::escape_html;
use crate::notify::config::TelegramConfig;
use crate::runner::Runner;

pub fn handle_bot_organize(config: &TelegramConfig) -> Result<String> {
    let http_client = Client::builder().timeout(Duration::from_secs(30)).build()?;
    let api_key = config.gemini_api_key.as_deref();

    let torrents = api::get_torrents(&http_client, &config.qbittorrent_url, None)?;
    let (completed, incomplete): (Vec<_>, Vec<_>) =
        torrents.into_iter().partition(TorrentInfo::is_completed);

    if completed.is_empty() {
        if !incomplete.is_empty() {
            return Ok(format!(
                "🌊 <b>領域 RYOIKI • Media Organizer</b>\n\
                ━━━━━━━━━━━━━━━━━━━━━━━\n\
                ℹ <b>No completed torrents to organize.</b>\n\
                <i>{} active download(s) in progress — waiting for completion.</i>",
                incomplete.len()
            ));
        }
        return Ok("🌊 <b>領域 RYOIKI • Media Organizer</b>\n\
            ━━━━━━━━━━━━━━━━━━━━━━━\n\
            No active or completed torrents found in qBittorrent."
            .to_string());
    }

    let mut results = Vec::new();
    let mut cleared = 0;

    for t in &completed {
        let res =
            crate::modules::media::organizer::organize_torrent(t, &http_client, api_key, false)?;
        if !res.is_empty()
            && api::delete_torrent(&http_client, &config.qbittorrent_url, &t.hash, false).is_ok()
        {
            cleared += 1;
        }
        results.extend(res);
    }

    if results.is_empty() {
        if cleared > 0 {
            return Ok(format!(
                "🌊 <b>領域 RYOIKI • Media Organizer</b>\n\
                ━━━━━━━━━━━━━━━━━━━━━━━\n\
                🗑 <b>Cleaned {cleared} torrent(s) from qBittorrent history.</b>"
            ));
        }
        return Ok("🌊 <b>領域 RYOIKI • Media Organizer</b>\n\
            ━━━━━━━━━━━━━━━━━━━━━━━\n\
            No new video files found to organize."
            .to_string());
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
        let clean = escape_html(&res.media_info.clean_name);
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

pub fn handle_bot_prune() -> Result<String> {
    crate::modules::media::pruner::handle_bot_prune()
}

pub fn handle_bot_sync() -> String {
    let mut messages: Vec<String> = Vec::new();
    match crate::modules::jellyfin::api::refresh_library_auto() {
        Ok(()) => messages.push("🔄 Jellyfin library scan triggered successfully.".to_string()),
        Err(e) => messages.push(format!("⚠️ Jellyfin library scan failed: {e}")),
    }

    let sync_res = std::thread::spawn(crate::modules::media::sync::run_media_sync).join();
    match sync_res {
        Ok(Ok(())) => messages.push("☁️ Cloud media sync completed successfully.".to_string()),
        Ok(Err(e)) => messages.push(format!("⚠️ Cloud media sync failed: {e}")),
        Err(_) => messages.push("⚠️ Cloud media sync thread panicked.".to_string()),
    }

    format!(
        "🌊 <b>領域 RYOIKI • Media Sync</b>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        {}\n\
        ━━━━━━━━━━━━━━━━━━━━━━━",
        messages.join("\n")
    )
}

pub fn handle_bot_audio(action: &str) -> Result<String> {
    if action == "retry" {
        let bin = crate::modules::media::audio::find_dubstrip_bin();
        if let Some(path) = bin {
            crate::modules::media::audio::strip_queue::process_queue(&path)?;
            Ok("🗡️ <b>Dubstrip:</b> Audio strip retry queue processed.".to_string())
        } else {
            Ok("❌ <b>Dubstrip:</b> <code>dubstrip</code> binary not found.".to_string())
        }
    } else {
        let is_active = crate::modules::media::audio::retry_timer::is_timer_active();
        let status = if is_active {
            "Active (hourly)"
        } else {
            "Inactive"
        };
        Ok(format!(
            "🌊 <b>領域 RYOIKI • DubStrip</b>\n\
            ━━━━━━━━━━━━━━━━━━━━━━━\n\
            Retry Timer: <b>{status}</b>\n\
            Use <code>/audio retry</code> to process pending dub strips immediately.\n\
            ━━━━━━━━━━━━━━━━━━━━━━━"
        ))
    }
}

pub fn handle_bot_check() -> String {
    let tools = [
        ("git", "Git VCS"),
        ("tmux", "Tmux Multiplexer"),
        ("nvim", "Neovim Editor"),
        ("gh", "GitHub CLI"),
        ("eza", "Eza ls"),
        ("bat", "Bat cat"),
        ("zoxide", "Zoxide cd"),
        ("fzf", "FZF finder"),
        ("go", "Go runtime"),
        ("rustc", "Rust compiler"),
        ("uv", "uv Python"),
        ("bun", "Bun JS/TS"),
        ("docker", "Docker Engine"),
        ("starship", "Starship prompt"),
        ("fastfetch", "Fastfetch stats"),
        ("toss", "toss-rs trash"),
        ("dubstrip", "DubStrip audio"),
        ("seedr-dl", "Seedr downloader"),
        ("tailscale", "Tailscale VPN"),
        ("rclone", "Rclone sync/mount"),
    ];

    let mut lines = vec![
        "🔍 <b>領域 RYOIKI • System Tool Audit</b>".to_string(),
        "━━━━━━━━━━━━━━━━━━━━━━━".to_string(),
    ];

    for (cmd, desc) in tools {
        let exists = Runner::command_exists(cmd);
        let status = if exists { "✔" } else { "✖" };
        lines.push(format!("{status} <code>{cmd:<10}</code> — {desc}"));
    }

    lines.push("━━━━━━━━━━━━━━━━━━━━━━━".to_string());
    lines.join("\n")
}
