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
use crate::notify::style;

/// Resolves a torrent page callback. Bare `cb:torrents` means page one;
/// `cb:torrents:<n>` selects that page. Range clamping happens in the renderer.
fn torrent_page(data: &str) -> Option<usize> {
    match data.strip_prefix("cb:torrents")? {
        "" => Some(1),
        rest => rest.strip_prefix(':')?.parse().ok(),
    }
}

pub fn handle_callback_query(
    client: &Client,
    config: &TelegramConfig,
    prompts: &crate::bot::prompts::Prompts,
    data: &str,
    msg_id: i64,
) -> Result<()> {
    if handle_seedrq_callback(client, config, prompts, data, msg_id) {
        return Ok(());
    }
    if handle_language_callback(client, config, data, msg_id) {
        return Ok(());
    }
    if handle_view_callback(client, config, data, msg_id)? {
        return Ok(());
    }
    handle_action_callback(client, config, data, msg_id)?;
    Ok(())
}

/// Handles the original-language buttons: `cb:al:<id>:<index>`.
fn handle_language_callback(
    client: &Client,
    config: &TelegramConfig,
    data: &str,
    msg_id: i64,
) -> bool {
    let Some(rest) = data.strip_prefix("cb:al:") else {
        return false;
    };
    let Some((id, action)) = rest.split_once(':') else {
        return false;
    };

    let text = if action == "silent" {
        match crate::modules::media::audio::approval::silence(id) {
            Ok(msg) => format!("{} {msg}", style::accent("·", "Silenced")),
            Err(e) => format!("{} {e}", style::accent("!", "Failed")),
        }
    } else {
        let Ok(index) = action.parse::<usize>() else {
            return false;
        };
        resolve_language_text(id, index)
    };

    let _ = edit_message(
        client,
        &config.bot_token,
        &config.chat_id,
        msg_id,
        &text,
        None,
    );
    true
}

/// Builds the reply text for a language-selection callback.
fn resolve_language_text(id: &str, index: usize) -> String {
    use crate::modules::media::audio::approval;
    let Some(entry) = approval::find_pending(id) else {
        return format!("{} Already resolved.", style::accent("✓", "No action"));
    };
    let Some(lang) = entry.candidates.get(index) else {
        return format!("{} Unknown track selection.", style::accent("!", "Invalid"));
    };
    let lang = lang.clone();
    match approval::resolve(id, &lang, false) {
        Ok(msg) => format!("{} {msg}", style::accent("✓", "Resolved")),
        Err(e) => format!("{} {e}", style::accent("!", "Failed")),
    }
}

/// Handles the Seedr queue prompt buttons: keep (default) or download first.
fn handle_seedrq_callback(
    client: &Client,
    config: &TelegramConfig,
    prompts: &crate::bot::prompts::Prompts,
    data: &str,
    msg_id: i64,
) -> bool {
    let Some(rest) = data.strip_prefix("cb:seedrq:") else {
        return false;
    };
    let Some((action, hash)) = rest.split_once(':') else {
        return false;
    };
    // User answered: cancel the timeout prompt.
    let _ = crate::bot::prompts::take(prompts, hash);

    let text = if action == "front" {
        crate::modules::torrent::scheduler::promote_front(hash);
        format!(
            "{}\n{}",
            style::accent("↑", "Moved to front of Seedr queue"),
            "<i>Downloads next when the cloud slot frees.</i>"
        )
    } else {
        format!(
            "{}\n{}",
            style::accent("✓", "Kept in Seedr queue"),
            "<i>Downloads automatically when the slot frees.</i>"
        )
    };
    let _ = edit_message(
        client,
        &config.bot_token,
        &config.chat_id,
        msg_id,
        &text,
        None,
    );
    true
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
        data if data == "cb:torrents" || data.starts_with("cb:torrents:") => {
            let page = torrent_page(data).unwrap_or(1);
            let (text, kb) = render_torrent_report(client, &config.qbittorrent_url, page)?;
            (text, Some(kb))
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

#[cfg(test)]
mod tests {
    use super::torrent_page;

    #[test]
    fn test_torrent_page_callback_parsing() {
        assert_eq!(torrent_page("cb:torrents"), Some(1));
        assert_eq!(torrent_page("cb:torrents:4"), Some(4));
        assert_eq!(torrent_page("cb:torrents:999"), Some(999));
        assert_eq!(torrent_page("cb:torrents:0"), Some(0));
        assert_eq!(torrent_page("cb:torrents:abc"), None);
        assert_eq!(torrent_page("cb:sys"), None);
    }
}
