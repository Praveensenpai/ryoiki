use anyhow::Result;
use reqwest::blocking::Client;

use super::actions::{execute_poweroff, execute_reboot};
use super::client::edit_message;
use super::maintenance::{
    handle_bot_audio, handle_bot_check, handle_bot_organize, handle_bot_prune, handle_bot_sync,
};
use super::services::{get_docker_containers, get_managed_services};
use super::system::collect_system_metrics;
use super::torrents::render_torrent_report;
use super::ui::{
    render_docker_view, render_maintenance_view, render_services_view, render_storage_view,
    render_system_view, render_unified_status,
};
use crate::notify::config::TelegramConfig;

pub fn handle_callback_query(
    client: &Client,
    config: &TelegramConfig,
    data: &str,
    msg_id: i64,
) -> Result<()> {
    if handle_view_callback(client, config, data, msg_id)? {
        return Ok(());
    }
    handle_action_callback(client, config, data, msg_id)?;
    Ok(())
}

fn handle_view_callback(
    client: &Client,
    config: &TelegramConfig,
    data: &str,
    msg_id: i64,
) -> Result<bool> {
    match data {
        "cb:status" => {
            let sys = collect_system_metrics(config);
            let torrents =
                super::torrents::get_torrents(client, &config.qbittorrent_url).unwrap_or_default();
            let containers = get_docker_containers().unwrap_or_default();
            let active_c = containers.iter().filter(|c| c.is_running).count();
            let (text, kb) = render_unified_status(&sys, torrents.len(), active_c);
            edit_message(
                client,
                &config.bot_token,
                &config.chat_id,
                msg_id,
                &text,
                Some(&kb),
            )?;
            Ok(true)
        }
        "cb:sys" => {
            let sys = collect_system_metrics(config);
            let (text, kb) = render_system_view(&sys);
            edit_message(
                client,
                &config.bot_token,
                &config.chat_id,
                msg_id,
                &text,
                Some(&kb),
            )?;
            Ok(true)
        }
        "cb:storage" => {
            let sys = collect_system_metrics(config);
            let (text, kb) = render_storage_view(&sys.disks, sys.gdrive_info.as_deref());
            edit_message(
                client,
                &config.bot_token,
                &config.chat_id,
                msg_id,
                &text,
                Some(&kb),
            )?;
            Ok(true)
        }
        "cb:docker" => {
            let containers = get_docker_containers().unwrap_or_default();
            let (text, kb) = render_docker_view(&containers);
            edit_message(
                client,
                &config.bot_token,
                &config.chat_id,
                msg_id,
                &text,
                Some(&kb),
            )?;
            Ok(true)
        }
        "cb:services" => {
            let services = get_managed_services();
            let (text, kb) = render_services_view(&services);
            edit_message(
                client,
                &config.bot_token,
                &config.chat_id,
                msg_id,
                &text,
                Some(&kb),
            )?;
            Ok(true)
        }
        "cb:torrents" => {
            let text = render_torrent_report(client, &config.qbittorrent_url)?;
            edit_message(
                client,
                &config.bot_token,
                &config.chat_id,
                msg_id,
                &text,
                None,
            )?;
            Ok(true)
        }
        "cb:maintenance" => {
            let (text, kb) = render_maintenance_view();
            edit_message(
                client,
                &config.bot_token,
                &config.chat_id,
                msg_id,
                &text,
                Some(&kb),
            )?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn handle_action_callback(
    client: &Client,
    config: &TelegramConfig,
    data: &str,
    msg_id: i64,
) -> Result<()> {
    match data {
        "cb:organize" => {
            let text = handle_bot_organize(config)?;
            edit_message(
                client,
                &config.bot_token,
                &config.chat_id,
                msg_id,
                &text,
                None,
            )?;
        }
        "cb:prune" => {
            let text = handle_bot_prune()?;
            edit_message(
                client,
                &config.bot_token,
                &config.chat_id,
                msg_id,
                &text,
                None,
            )?;
        }
        "cb:sync" => {
            let text = handle_bot_sync();
            edit_message(
                client,
                &config.bot_token,
                &config.chat_id,
                msg_id,
                &text,
                None,
            )?;
        }
        "cb:audio" => {
            let text = handle_bot_audio("retry")?;
            edit_message(
                client,
                &config.bot_token,
                &config.chat_id,
                msg_id,
                &text,
                None,
            )?;
        }
        "cb:check" => {
            let text = handle_bot_check();
            edit_message(
                client,
                &config.bot_token,
                &config.chat_id,
                msg_id,
                &text,
                None,
            )?;
        }
        "cb:reboot_confirm" => {
            let text = execute_reboot()?;
            edit_message(
                client,
                &config.bot_token,
                &config.chat_id,
                msg_id,
                &text,
                None,
            )?;
        }
        "cb:poweroff_confirm" => {
            let text = execute_poweroff()?;
            edit_message(
                client,
                &config.bot_token,
                &config.chat_id,
                msg_id,
                &text,
                None,
            )?;
        }
        "cb:cancel" => {
            let _ = handle_view_callback(client, config, "cb:status", msg_id);
        }
        _ => {}
    }
    Ok(())
}
