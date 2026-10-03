use anyhow::Result;
use reqwest::blocking::Client;

use super::actions::{handle_charge_limit, handle_self_update};
use super::callbacks::handle_callback_query;
use super::client::{download_telegram_file, reply, reply_with_keyboard};
use super::maintenance::{
    handle_bot_audio, handle_bot_check, handle_bot_organize, handle_bot_prune, handle_bot_sync,
};
use super::services::{
    get_docker_containers, get_docker_logs, get_managed_services, restart_docker_container,
    restart_managed_service,
};
use super::system::collect_system_metrics;
use super::torrents::{
    handle_magnet, handle_seedr_cmd, handle_torrent_file, pause_all, render_torrent_report,
    resume_all,
};
use super::types::{CallbackQuery, Message};
use super::ui::{
    render_docker_view, render_poweroff_confirm, render_reboot_confirm, render_services_view,
    render_storage_view, render_system_view, render_unified_status,
};
use crate::notify::client::escape_html;
use crate::notify::config::TelegramConfig;

pub fn route_message(client: &Client, config: &TelegramConfig, msg: Message) {
    let Ok(auth_id) = config.chat_id.parse::<i64>() else {
        return;
    };
    if msg.from.is_none_or(|u| u.id != auth_id) {
        return;
    }

    if let Some(doc) = msg.document {
        handle_torrent_document(client, config, &doc);
        return;
    }

    let Some(text) = msg.text else {
        return;
    };
    let trimmed = text.trim();
    if trimmed.starts_with("magnet:?xt=urn:") {
        let res = handle_magnet(client, config, trimmed);
        let _ = reply(client, &config.bot_token, &config.chat_id, &res);
        return;
    }

    if let Some((cmd, full_text)) = extract_command(trimmed) {
        if let Err(e) = dispatch_command(client, config, cmd, full_text) {
            let clean_err = escape_html(&e.to_string());
            let err_msg = format!("❌ <b>Command Failed ({cmd}):</b>\n<code>{clean_err}</code>");
            let _ = reply(client, &config.bot_token, &config.chat_id, &err_msg);
        }
    }
}

fn handle_torrent_document(client: &Client, config: &TelegramConfig, doc: &super::types::Document) {
    let fname = doc
        .file_name
        .clone()
        .unwrap_or_else(|| "download.torrent".to_string());
    if !fname.ends_with(".torrent") {
        return;
    }
    let res = download_telegram_file(client, &config.bot_token, &doc.file_id)
        .and_then(|(_, bytes)| handle_torrent_file(client, config, &fname, bytes));
    let msg = match res {
        Ok(text) => text,
        Err(e) => format!("❌ <b>Failed to add torrent:</b> {e}"),
    };
    let _ = reply(client, &config.bot_token, &config.chat_id, &msg);
}

pub fn route_callback(client: &Client, config: &TelegramConfig, cb: CallbackQuery) {
    let Ok(auth_id) = config.chat_id.parse::<i64>() else {
        return;
    };
    if cb.from.id != auth_id {
        return;
    }

    let Some(data) = cb.data else {
        return;
    };
    let Some(msg) = cb.message else {
        return;
    };

    let _ = super::client::answer_callback(client, &config.bot_token, &cb.id, None);
    if let Err(e) = handle_callback_query(client, config, &data, msg.id) {
        eprintln!("[bot] Callback error ({data}): {e}");
    }
}

fn dispatch_telemetry(client: &Client, config: &TelegramConfig, cmd: &str) -> Result<bool> {
    match cmd {
        "/status" => {
            send_status_dashboard(client, config)?;
            Ok(true)
        }
        "/system" | "/sys" => {
            let sys = collect_system_metrics(config);
            let (text, kb) = render_system_view(&sys);
            reply_with_keyboard(client, &config.bot_token, &config.chat_id, &text, &kb)?;
            Ok(true)
        }
        "/storage" | "/disk" => {
            let sys = collect_system_metrics(config);
            let (text, kb) = render_storage_view(&sys.disks, sys.gdrive_info.as_deref());
            reply_with_keyboard(client, &config.bot_token, &config.chat_id, &text, &kb)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn dispatch_media(
    client: &Client,
    config: &TelegramConfig,
    cmd: &str,
    full_text: &str,
) -> Result<bool> {
    match cmd {
        "/docker" | "/ps" => handle_docker_cmd(client, config, full_text).map(|()| true),
        "/services" | "/service" => handle_services_cmd(client, config, full_text).map(|()| true),
        "/torrent" | "/torrents" => {
            let text = render_torrent_report(client, &config.qbittorrent_url)?;
            reply(client, &config.bot_token, &config.chat_id, &text).map(|()| true)
        }
        "/seedr" => {
            let arg = full_text.strip_prefix("/seedr").unwrap_or("").trim();
            let text = handle_seedr_cmd(arg, config.api_port);
            reply(client, &config.bot_token, &config.chat_id, &text).map(|()| true)
        }
        "/pause" => {
            pause_all(client, &config.qbittorrent_url)?;
            reply(
                client,
                &config.bot_token,
                &config.chat_id,
                "⏸ <b>All torrents paused</b>",
            )
            .map(|()| true)
        }
        "/resume" => {
            resume_all(client, &config.qbittorrent_url)?;
            reply(
                client,
                &config.bot_token,
                &config.chat_id,
                "▶️ <b>All torrents resumed</b>",
            )
            .map(|()| true)
        }
        "/organize" | "/organise" => {
            let text = handle_bot_organize(config)?;
            reply(client, &config.bot_token, &config.chat_id, &text).map(|()| true)
        }
        "/prune" => {
            let text = handle_bot_prune()?;
            reply(client, &config.bot_token, &config.chat_id, &text).map(|()| true)
        }
        "/sync" | "/refresh" => {
            let text = handle_bot_sync();
            reply(client, &config.bot_token, &config.chat_id, &text).map(|()| true)
        }
        "/audio" => {
            let arg = full_text.strip_prefix("/audio").unwrap_or("").trim();
            let text = handle_bot_audio(arg)?;
            reply(client, &config.bot_token, &config.chat_id, &text).map(|()| true)
        }
        _ => Ok(false),
    }
}

fn dispatch_control(
    client: &Client,
    config: &TelegramConfig,
    cmd: &str,
    full_text: &str,
) -> Result<bool> {
    match cmd {
        "/charge" => {
            let arg = full_text.strip_prefix("/charge").unwrap_or("").trim();
            let text = handle_charge_limit(arg);
            reply(client, &config.bot_token, &config.chat_id, &text).map(|()| true)
        }
        "/check" => {
            let text = handle_bot_check();
            reply(client, &config.bot_token, &config.chat_id, &text).map(|()| true)
        }
        "/reboot" => {
            let (text, kb) = render_reboot_confirm();
            reply_with_keyboard(client, &config.bot_token, &config.chat_id, &text, &kb)
                .map(|()| true)
        }
        "/poweroff" => {
            let (text, kb) = render_poweroff_confirm();
            reply_with_keyboard(client, &config.bot_token, &config.chat_id, &text, &kb)
                .map(|()| true)
        }
        "/update" => {
            let text = handle_self_update();
            reply(client, &config.bot_token, &config.chat_id, &text).map(|()| true)
        }
        "/help" | "/start" => send_help(client, config).map(|()| true),
        _ => Ok(false),
    }
}

fn dispatch_command(
    client: &Client,
    config: &TelegramConfig,
    cmd: &str,
    full_text: &str,
) -> Result<()> {
    if dispatch_telemetry(client, config, cmd)?
        || dispatch_media(client, config, cmd, full_text)?
        || dispatch_control(client, config, cmd, full_text)?
    {
        return Ok(());
    }

    reply(
        client,
        &config.bot_token,
        &config.chat_id,
        "❓ <b>Unknown command.</b> Send /help to view command list.",
    )
}

fn send_status_dashboard(client: &Client, config: &TelegramConfig) -> Result<()> {
    let sys = collect_system_metrics(config);
    let torrents =
        super::torrents::get_torrents(client, &config.qbittorrent_url).unwrap_or_default();
    let containers = get_docker_containers().unwrap_or_default();
    let active_c = containers.iter().filter(|c| c.is_running).count();
    let (text, kb) = render_unified_status(&sys, torrents.len(), active_c);
    reply_with_keyboard(client, &config.bot_token, &config.chat_id, &text, &kb)
}

fn handle_docker_cmd(client: &Client, config: &TelegramConfig, full: &str) -> Result<()> {
    let args: Vec<&str> = full.split_whitespace().collect();
    if args.len() >= 3 && args[1] == "restart" {
        let res = restart_docker_container(args[2])?;
        reply(client, &config.bot_token, &config.chat_id, &res)?;
    } else if args.len() >= 3 && args[1] == "logs" {
        let logs = get_docker_logs(args[2], 25)?;
        let clean = escape_html(&logs);
        let res = format!("🐳 <b>Logs ({})</b>:\n<code>{clean}</code>", args[2]);
        reply(client, &config.bot_token, &config.chat_id, &res)?;
    } else {
        let containers = get_docker_containers()?;
        let (text, kb) = render_docker_view(&containers);
        reply_with_keyboard(client, &config.bot_token, &config.chat_id, &text, &kb)?;
    }
    Ok(())
}

fn handle_services_cmd(client: &Client, config: &TelegramConfig, full: &str) -> Result<()> {
    let args: Vec<&str> = full.split_whitespace().collect();
    if args.len() >= 3 && args[1] == "restart" {
        let res = restart_managed_service(args[2])?;
        reply(client, &config.bot_token, &config.chat_id, &res)?;
    } else {
        let services = get_managed_services();
        let (text, kb) = render_services_view(&services);
        reply_with_keyboard(client, &config.bot_token, &config.chat_id, &text, &kb)?;
    }
    Ok(())
}

fn send_help(client: &Client, config: &TelegramConfig) -> Result<()> {
    let help_text = "🌊 <b>領域 RYOIKI • Command Center</b>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        📊 <b>System Telemetry:</b>\n\
        /status — Unified server dashboard with action buttons\n\
        /system — CPU load, RAM, thermals, IPs, battery\n\
        /storage — NVMe, local partitions, & Google Drive\n\n\
        🐳 <b>Containers & Services:</b>\n\
        /docker — List Docker container states & ports\n\
        /docker restart &lt;name&gt; — Restart container\n\
        /services — Background systemd services & timers\n\
        /service restart &lt;name&gt; — Restart systemd service\n\n\
        📦 <b>Torrents & Media:</b>\n\
        /torrent — Active downloads, speeds, ETAs\n\
        /seedr &lt;magnet&gt; — Offload torrent to Seedr cloud\n\
        /pause • /resume — Torrent controls\n\
        /organize — Classify & move media to Jellyfin\n\
        /prune — Evict watched media to Google Drive\n\
        /sync — Jellyfin refresh & cloud sync\n\
        /audio [retry] — Dubstrip audio queue runner\n\n\
        ⚙️ <b>Server Controls:</b>\n\
        /charge [limit] — View or set battery charge limit\n\
        /reboot — Graceful server restart (with confirmation)\n\
        /poweroff — Graceful server shutdown (with confirmation)\n\
        /check — Audit system tools & dev runtimes\n\
        /update — Update ryoiki to latest release\n\
        ━━━━━━━━━━━━━━━━━━━━━━━";
    reply(client, &config.bot_token, &config.chat_id, help_text)
}

pub fn extract_command(text: &str) -> Option<(&str, &str)> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    let full_cmd = trimmed.split_whitespace().next()?;
    let cmd = full_cmd.split('@').next().unwrap_or(full_cmd);
    Some((cmd, trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_command_standard() {
        assert_eq!(extract_command("/status"), Some(("/status", "/status")));
        assert_eq!(
            extract_command("/system --verbose"),
            Some(("/system", "/system --verbose"))
        );
        assert_eq!(
            extract_command("  /docker ps  "),
            Some(("/docker", "/docker ps"))
        );
    }

    #[test]
    fn test_extract_command_with_bot_username() {
        assert_eq!(
            extract_command("/status@ryoiki_bot"),
            Some(("/status", "/status@ryoiki_bot"))
        );
        assert_eq!(
            extract_command("/service@ryoiki_bot restart docker"),
            Some(("/service", "/service@ryoiki_bot restart docker"))
        );
    }

    #[test]
    fn test_extract_command_non_command() {
        assert_eq!(extract_command("hello world"), None);
        assert_eq!(extract_command("magnet:?xt=urn:btih:..."), None);
        assert_eq!(extract_command(""), None);
    }
}
