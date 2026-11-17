use anyhow::Result;
use reqwest::blocking::Client;
use std::time::Duration;

use crate::modules::torrent::api::{self, TorrentInfo};
use crate::notify::client::escape_html;
use crate::notify::config::TelegramConfig;
use crate::notify::style;
use crate::runner::Runner;

const TRASH_REAP_INTERVAL: Duration = Duration::from_hours(1);

/// Background reaper that purges trash entries older than the retention window.
pub fn start_trash_reaper() {
    std::thread::spawn(|| loop {
        match crate::modules::media::trash::purge_expired() {
            Ok(0) => {}
            Ok(n) => println!("  🧹 Purged {n} expired trash item(s)"),
            Err(e) => eprintln!("  ⚠ Trash reaper failed: {e:#}"),
        }
        std::thread::sleep(TRASH_REAP_INTERVAL);
    });
}

pub fn handle_bot_organize(config: &TelegramConfig) -> Result<String> {
    let http_client = Client::builder().timeout(Duration::from_secs(30)).build()?;
    let api_key = config.gemini_api_key.as_deref();

    let torrents = api::get_torrents(&http_client, &config.qbittorrent_url, None)?;
    let (completed, incomplete): (Vec<_>, Vec<_>) =
        torrents.into_iter().partition(TorrentInfo::is_completed);

    if completed.is_empty() {
        if !incomplete.is_empty() {
            return Ok(format!(
                "{}\n\n{}\n\n<i>{} active download(s) in progress; waiting for completion.</i>",
                style::view("Media Organizer"),
                style::accent("·", "No completed torrents to organize"),
                incomplete.len()
            ));
        }
        return Ok(format!(
            "{}\n\nNo active or completed torrents found in qBittorrent.",
            style::view("Media Organizer")
        ));
    }

    let mut results = Vec::new();
    let mut cleared = 0;

    for t in &completed {
        let res =
            crate::modules::media::organizer::organize_torrent(t, &http_client, api_key, false)?;
        if !res.is_empty()
            && api::delete_torrent(&http_client, &config.qbittorrent_url, &t.hash, true).is_ok()
        {
            cleared += 1;
        }
        results.extend(res);
    }

    Ok(format_organize_report(&results, cleared))
}

fn format_organize_report(
    results: &[crate::modules::media::OrganizeResult],
    cleared: usize,
) -> String {
    if results.is_empty() {
        if cleared > 0 {
            return format!(
                "{}\n\n{}",
                style::view("Media Organizer"),
                style::accent("✓", &format!("Cleaned {cleared} torrent(s) from history"))
            );
        }
        return format!(
            "{}\n\nNo new video files found to organize.",
            style::view("Media Organizer")
        );
    }

    let mut lines = vec![
        style::view("Media Organizer"),
        String::new(),
        style::accent(
            "✓",
            &format!("Organized {} item(s) into Jellyfin", results.len()),
        ),
    ];

    for res in results.iter().take(5) {
        let clean = escape_html(&res.media_info.clean_name);
        lines.push(format!(
            "{}<code>{clean}</code>  <i>{}</i>",
            style::key("MEDIA"),
            res.media_info.engine
        ));
    }

    if results.len() > 5 {
        lines.push(format!("<i>...and {} more items</i>", results.len() - 5));
    }

    let mut audio = crate::modules::media::audio::AudioStripSummary::default();
    for res in results {
        audio.merge(&res.audio_summary);
    }
    if let Some(line) = audio.render_html_line() {
        lines.push(String::new());
        lines.push(line);
    }

    if cleared > 0 {
        lines.push(String::new());
        lines.push(format!(
            "<i>Removed {cleared} torrent(s) from qBittorrent</i>"
        ));
    }

    lines.join("\n")
}

pub fn handle_bot_library() -> Vec<String> {
    let report = crate::modules::media::library::scan_library(
        &crate::modules::media::organizer::pathing::get_jellyfin_media_dir(),
    );
    report.render_pages()
}

pub fn handle_bot_prune() -> Result<String> {
    crate::modules::media::pruner::handle_bot_prune()
}

pub fn handle_bot_sync() -> String {
    let mut messages: Vec<String> = Vec::new();
    match crate::modules::jellyfin::api::refresh_library_auto() {
        Ok(()) => messages.push("Jellyfin library scan triggered.".to_string()),
        Err(e) => messages.push(format!("{} {e}", style::accent("!", "Scan failed"))),
    }

    let sync_res = std::thread::spawn(crate::modules::media::sync::run_media_sync).join();
    match sync_res {
        Ok(Ok(())) => messages.push("Cloud media sync completed.".to_string()),
        Ok(Err(e)) => messages.push(format!("{} {e}", style::accent("!", "Sync failed"))),
        Err(_) => messages.push(style::accent("!", "Sync thread panicked")),
    }

    format!("{}\n\n{}", style::view("Media Sync"), messages.join("\n"))
}

pub fn handle_bot_audio(action: &str) -> Result<String> {
    if action == "pending" {
        return Ok(format!(
            "{}\n\n{}",
            style::view("Pending Decisions"),
            escape_html(&crate::modules::media::audio::approval::list_pending_text())
        ));
    }
    if action == "retry" {
        let bin = crate::modules::media::audio::find_dubstrip_bin();
        if let Some(path) = bin {
            let summary = crate::modules::media::audio::strip_queue::process_queue(&path)?;
            let mut msg = format!(
                "{}\n{}",
                style::view("Dubstrip"),
                style::accent("✓", "Audio strip retry queue processed")
            );
            if let Some(line) = summary.render_html_line() {
                msg.push('\n');
                msg.push_str(&line);
            }
            Ok(msg)
        } else {
            Ok(format!(
                "{}\n{}",
                style::view("Dubstrip"),
                style::accent("!", "<code>dubstrip</code> binary not found")
            ))
        }
    } else {
        let is_active = crate::modules::media::audio::retry_timer::is_timer_active();
        let pending_active = crate::modules::media::audio::pending_timer::is_timer_active();
        let status = if is_active {
            "Active (hourly)"
        } else {
            "Inactive"
        };
        let pending_status = if pending_active {
            "Active (daily)"
        } else {
            "Inactive"
        };
        Ok(format!(
            "{}\n\n{}{}\n{}{}\n\n<i>Run <code>/audio retry</code> to process the queue immediately.</i>",
            style::view("DubStrip"),
            style::key("Retry Timer:"),
            format_args!("<b>{status}</b>"),
            style::key("Pending Timer:"),
            format_args!("<b>{pending_status}</b>")
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
        ("opencode", "OpenCode AI agent"),
        ("tailscale", "Tailscale VPN"),
        ("rclone", "Rclone sync/mount"),
    ];

    let mut lines = vec![style::view("System Tool Audit"), String::new()];

    for (cmd, desc) in tools {
        let exists = Runner::command_exists(cmd);
        let marker = if exists { "●" } else { "○" };
        lines.push(format!(
            "{marker} {}{}  <i>{desc}</i>",
            style::key(cmd),
            if exists { "" } else { " missing" }
        ));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_bot_check_contains_core_tools() {
        let text = handle_bot_check();
        assert!(text.contains("System Tool Audit"));
        assert!(text.contains("git"));
        assert!(text.contains("docker"));
    }

    #[test]
    fn test_handle_bot_audio_status() {
        let res = handle_bot_audio("");
        assert!(res.is_ok());
        let text = res.unwrap_or_default();
        assert!(text.contains("DubStrip"));
        assert!(text.contains("Retry Timer:"));
    }
}
