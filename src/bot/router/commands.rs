use anyhow::Result;
use reqwest::blocking::Client;

use super::super::actions::{handle_charge_limit, handle_self_update};
use super::super::client::{reply, reply_with_keyboard};
use super::super::maintenance::{
    handle_bot_audio, handle_bot_check, handle_bot_library, handle_bot_organize, handle_bot_prune,
    handle_bot_sync,
};
use super::super::services::{
    get_docker_containers, get_docker_logs, get_managed_services, restart_docker_container,
    restart_managed_service,
};
use super::super::system::collect_system_metrics;
use super::super::torrents::{handle_seedr_cmd, pause_all, render_torrent_report, resume_all};
use super::super::ui::{
    render_docker_view, render_poweroff_confirm, render_reboot_confirm, render_services_view,
    render_storage_view, render_system_view, render_unified_status,
};
use crate::notify::client::escape_html;
use crate::notify::config::TelegramConfig;
use crate::notify::style;

fn command_arg(full_text: &str) -> &str {
    full_text
        .split_whitespace()
        .next()
        .map_or("", |first| full_text[first.len()..].trim())
}

pub(super) fn dispatch_command(
    client: &Client,
    config: &TelegramConfig,
    prompts: &crate::bot::prompts::Prompts,
    (cmd, full_text): (&str, &str),
) -> Result<()> {
    if dispatch_telemetry(client, config, cmd)?
        || dispatch_torrent(client, config, prompts, (cmd, full_text))?
        || dispatch_media(client, config, (cmd, full_text))?
        || dispatch_control(client, config, (cmd, full_text))?
    {
        return Ok(());
    }

    reply(
        client,
        &config.bot_token,
        &config.chat_id,
        &format!(
            "{} Unknown command. Send /help for the command list.",
            style::accent("?", "Unknown")
        ),
    )
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

fn dispatch_torrent(
    client: &Client,
    config: &TelegramConfig,
    prompts: &crate::bot::prompts::Prompts,
    (cmd, full_text): (&str, &str),
) -> Result<bool> {
    match cmd {
        "/torrent" | "/torrents" => {
            let arg = command_arg(full_text);
            if arg.is_empty() {
                let (text, kb) = render_torrent_report(client, &config.qbittorrent_url, 1)?;
                reply_with_keyboard(client, &config.bot_token, &config.chat_id, &text, &kb)?;
            } else {
                let mag = super::extract_magnet(arg).unwrap_or_else(|| arg.to_string());
                super::handle_magnet_message(client, config, prompts, &mag);
            }
            Ok(true)
        }
        "/add" | "/magnet" => {
            let arg = command_arg(full_text);
            if arg.is_empty() {
                reply(
                    client,
                    &config.bot_token,
                    &config.chat_id,
                    &format!(
                        "{}<code>/magnet &lt;magnet-url-or-hash&gt;</code>",
                        style::key("USAGE")
                    ),
                )?;
            } else {
                let mag = super::extract_magnet(arg).unwrap_or_else(|| arg.to_string());
                super::handle_magnet_message(client, config, prompts, &mag);
            }
            Ok(true)
        }
        "/seedr" => {
            let arg = command_arg(full_text);
            let text = handle_seedr_cmd(client, config, arg);
            reply(client, &config.bot_token, &config.chat_id, &text).map(|()| true)
        }
        "/pause" => {
            pause_all(client, &config.qbittorrent_url)?;
            reply(
                client,
                &config.bot_token,
                &config.chat_id,
                &style::accent("‖", "All torrents paused"),
            )
            .map(|()| true)
        }
        "/resume" => {
            resume_all(client, &config.qbittorrent_url)?;
            reply(
                client,
                &config.bot_token,
                &config.chat_id,
                &style::accent("→", "All torrents resumed"),
            )
            .map(|()| true)
        }
        _ => Ok(false),
    }
}

fn dispatch_media(
    client: &Client,
    config: &TelegramConfig,
    (cmd, full_text): (&str, &str),
) -> Result<bool> {
    match cmd {
        "/docker" | "/ps" => handle_docker_cmd(client, config, full_text).map(|()| true),
        "/services" | "/service" => handle_services_cmd(client, config, full_text).map(|()| true),
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
            let arg = command_arg(full_text);
            let text = handle_bot_audio(arg)?;
            reply(client, &config.bot_token, &config.chat_id, &text).map(|()| true)
        }
        "/movies" | "/library" => {
            for page in handle_bot_library() {
                reply(client, &config.bot_token, &config.chat_id, &page)?;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn dispatch_control(
    client: &Client,
    config: &TelegramConfig,
    (cmd, full_text): (&str, &str),
) -> Result<bool> {
    match cmd {
        "/charge" => {
            let arg = command_arg(full_text);
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
        "/request" => reply(
            client,
            &config.bot_token,
            &config.chat_id,
            &style::accent("✓", "Access already granted"),
        )
        .map(|()| true),
        _ => Ok(false),
    }
}

fn send_status_dashboard(client: &Client, config: &TelegramConfig) -> Result<()> {
    let sys = collect_system_metrics(config);
    let torrents =
        super::super::torrents::get_torrents(client, &config.qbittorrent_url).unwrap_or_default();
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
        let res = format!(
            "{}<code>{clean}</code>",
            style::key(&format!("LOGS {}", args[2]))
        );
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

fn help_sections() -> [(&'static str, &'static [(&'static str, &'static str)]); 4] {
    [
        (
            "TELEMETRY",
            &[
                ("/status", "unified server dashboard"),
                ("/system", "cpu, memory, thermals, network"),
                ("/storage", "disks and cloud storage"),
            ],
        ),
        (
            "CONTAINERS",
            &[
                ("/docker", "list containers and ports"),
                ("/docker restart <name>", "restart a container"),
                ("/service restart <name>", "restart a systemd unit"),
                ("/services", "list managed services"),
            ],
        ),
        (
            "TORRENTS & MEDIA",
            &[
                ("/torrents", "paginated torrent list"),
                ("/magnet <link>", "queue a magnet"),
                ("/seedr [link]", "offload to Seedr cloud"),
                ("/pause /resume", "torrent controls"),
                ("/movies", "Jellyfin library index"),
                ("/organize", "ingest media into Jellyfin"),
                ("/prune", "archive watched media"),
                ("/sync", "refresh Jellyfin and cloud"),
                ("/audio [retry|pending]", "dubstrip decisions"),
            ],
        ),
        (
            "SERVER",
            &[
                ("/charge [limit]", "battery charge limit"),
                ("/reboot", "graceful restart"),
                ("/poweroff", "graceful shutdown"),
                ("/check", "audit installed tools"),
                ("/update", "update ryoiki"),
                ("/request", "request access (non-owners)"),
            ],
        ),
    ]
}

fn send_help(client: &Client, config: &TelegramConfig) -> Result<()> {
    let mut lines = vec![style::view("COMMANDS"), String::new()];
    for (section, entries) in help_sections() {
        lines.push(style::eyebrow(section));
        for (cmd, description) in entries {
            lines.push(format!(
                "{}<i>{description}</i>",
                style::key(&escape_html(cmd))
            ));
        }
        lines.push(String::new());
    }
    lines.pop();
    reply(
        client,
        &config.bot_token,
        &config.chat_id,
        &lines.join("\n"),
    )
}
