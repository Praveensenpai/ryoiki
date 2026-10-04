use reqwest::blocking::Client;

use super::callbacks::handle_callback_query;
use super::client::{download_telegram_file, reply};
use super::torrents::{handle_magnet, handle_torrent_file, MagnetOutcome};
use super::types::{CallbackQuery, Message};
use crate::notify::client::escape_html;
use crate::notify::config::TelegramConfig;

mod commands;

use commands::dispatch_command;

pub fn route_message(
    client: &Client,
    config: &TelegramConfig,
    prompts: &crate::bot::prompts::Prompts,
    msg: Message,
) {
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
        handle_magnet_message(client, config, prompts, trimmed);
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

fn handle_magnet_message(
    client: &Client,
    config: &TelegramConfig,
    prompts: &crate::bot::prompts::Prompts,
    magnet: &str,
) {
    match handle_magnet(client, config, magnet) {
        MagnetOutcome::Message(text) => {
            let _ = reply(client, &config.bot_token, &config.chat_id, &text);
        }
        MagnetOutcome::Prompt { text, hash } => {
            let keyboard = crate::bot::keyboards::seedr_queue_keyboard(&hash);
            let msg_id = super::client::reply_with_keyboard_id(
                client,
                &config.bot_token,
                &config.chat_id,
                &text,
                &keyboard,
            );
            if let Ok(msg_id) = msg_id {
                crate::bot::prompts::register(prompts, &hash, msg_id);
                let client = client.clone();
                let cfg = config.clone();
                crate::bot::prompts::spawn_timeout(
                    prompts,
                    &hash,
                    config.seedr_prompt_timeout_secs,
                    move |msg_id| {
                        let _ = super::client::edit_message(
                            &client,
                            &cfg.bot_token,
                            &cfg.chat_id,
                            msg_id,
                            crate::bot::prompts::AUTO_QUEUED_TEXT,
                            None,
                        );
                    },
                );
            }
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

pub fn route_callback(
    client: &Client,
    config: &TelegramConfig,
    prompts: &crate::bot::prompts::Prompts,
    cb: CallbackQuery,
) {
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
    if let Err(e) = handle_callback_query(client, config, prompts, &data, msg.id) {
        eprintln!("[bot] Callback error ({data}): {e}");
    }
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
