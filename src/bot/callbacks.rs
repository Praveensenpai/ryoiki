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

fn resolve_view(
    client: &Client,
    config: &TelegramConfig,
    data: &str,
) -> Result<Option<(String, Option<crate::bot::types::InlineKeyboardMarkup>)>> {
    let res = match data {
        "cb:status" => {
            let sys = collect_system_metrics(config);
            let torrents =
                super::torrents::get_torrents(client, &config.qbittorrent_url).unwrap_or_default();
            let containers = get_docker_containers().unwrap_or_default();
            let active_c = containers.iter().filter(|c| c.is_running).count();
            let (text, kb) = render_unified_status(&sys, torrents.len(), active_c);
            (text, Some(kb))
        }
        "cb:sys" => {
            let sys = collect_system_metrics(config);
            let (text, kb) = render_system_view(&sys);
            (text, Some(kb))
        }
        "cb:storage" => {
            let sys = collect_system_metrics(config);
            let (text, kb) = render_storage_view(&sys.disks, sys.gdrive_info.as_deref());
            (text, Some(kb))
        }
        "cb:docker" => {
            let containers = get_docker_containers().unwrap_or_default();
            let (text, kb) = render_docker_view(&containers);
            (text, Some(kb))
        }
        "cb:services" => {
            let services = get_managed_services();
            let (text, kb) = render_services_view(&services);
            (text, Some(kb))
        }
        "cb:torrents" => {
            let text = render_torrent_report(client, &config.qbittorrent_url)?;
            (text, None)
        }
        "cb:maintenance" => {
            let (text, kb) = render_maintenance_view();
            (text, Some(kb))
        }
        _ => return Ok(None),
    };
    Ok(Some(res))
}

fn handle_view_callback(
    client: &Client,
    config: &TelegramConfig,
    data: &str,
    msg_id: i64,
) -> Result<bool> {
    let Some((text, kb)) = resolve_view(client, config, data)? else {
        return Ok(false);
    };
    edit_message(
        client,
        &config.bot_token,
        &config.chat_id,
        msg_id,
        &text,
        kb.as_ref(),
    )?;
    Ok(true)
}

fn resolve_action(config: &TelegramConfig, data: &str) -> Result<Option<String>> {
    let text = match data {
        "cb:organize" => handle_bot_organize(config)?,
        "cb:prune" => handle_bot_prune()?,
        "cb:sync" => handle_bot_sync(),
        "cb:audio" => handle_bot_audio("retry")?,
        "cb:check" => handle_bot_check(),
        "cb:reboot_confirm" => execute_reboot()?,
        "cb:poweroff_confirm" => execute_poweroff()?,
        _ => return Ok(None),
    };
    Ok(Some(text))
}

fn handle_action_callback(
    client: &Client,
    config: &TelegramConfig,
    data: &str,
    msg_id: i64,
) -> Result<()> {
    if data == "cb:cancel" {
        let _ = handle_view_callback(client, config, "cb:status", msg_id);
        return Ok(());
    }

    if let Some(text) = resolve_action(config, data)? {
        edit_message(
            client,
            &config.bot_token,
            &config.chat_id,
            msg_id,
            &text,
            None,
        )?;
    }
    Ok(())
}
