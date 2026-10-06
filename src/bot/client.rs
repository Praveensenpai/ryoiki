use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde_json::json;

use super::types::{FileResult, InlineKeyboardMarkup, TelegramResponse, Update};
use crate::notify::client::finalize_telegram_message;

pub fn fetch_updates(client: &Client, token: &str, offset: i64) -> Result<Vec<Update>> {
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

pub fn reply(client: &Client, token: &str, chat_id: &str, text: &str) -> Result<()> {
    crate::notify::client::send_telegram_alert(client, token, chat_id, text)
}

pub fn reply_with_keyboard(
    client: &Client,
    token: &str,
    chat_id: &str,
    text: &str,
    keyboard: &InlineKeyboardMarkup,
) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    let text = finalize_telegram_message(text);
    let payload = json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "HTML",
        "reply_markup": keyboard,
    });

    let resp = client.post(&url).json(&payload).send()?;
    if !resp.status().is_success() {
        let err_text = resp.text().unwrap_or_default();
        anyhow::bail!("Telegram sendMessage failed: {err_text}");
    }
    Ok(())
}

/// Sends a message with an inline keyboard and returns the new message id.
pub fn reply_with_keyboard_id(
    client: &Client,
    token: &str,
    chat_id: &str,
    text: &str,
    keyboard: &InlineKeyboardMarkup,
) -> Result<i64> {
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    let text = finalize_telegram_message(text);
    let payload = json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "HTML",
        "reply_markup": keyboard,
    });
    let resp: serde_json::Value = client.post(&url).json(&payload).send()?.json()?;
    resp["result"]["message_id"]
        .as_i64()
        .context("Telegram sendMessage response missing message_id")
}

pub fn edit_message(
    client: &Client,
    token: &str,
    chat_id: &str,
    message_id: i64,
    text: &str,
    keyboard: Option<&InlineKeyboardMarkup>,
) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{token}/editMessageText");
    let text = finalize_telegram_message(text);
    let mut payload = json!({
        "chat_id": chat_id,
        "message_id": message_id,
        "text": text,
        "parse_mode": "HTML",
    });

    if let Some(kb) = keyboard {
        payload["reply_markup"] = json!(kb);
    }

    let resp = client.post(&url).json(&payload).send()?;
    if !resp.status().is_success() {
        let err_text = resp.text().unwrap_or_default();
        // Ignore errors when message content is not modified
        if !err_text.contains("message is not modified") {
            anyhow::bail!("Telegram editMessageText failed: {err_text}");
        }
    }
    Ok(())
}

pub fn answer_callback(
    client: &Client,
    token: &str,
    callback_id: &str,
    toast: Option<&str>,
) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{token}/answerCallbackQuery");
    let mut payload = json!({
        "callback_query_id": callback_id,
    });
    if let Some(t) = toast {
        payload["text"] = json!(t);
    }

    client.post(&url).json(&payload).send()?;
    Ok(())
}

pub fn download_telegram_file(
    client: &Client,
    token: &str,
    file_id: &str,
) -> Result<(String, Vec<u8>)> {
    let file_url = format!("https://api.telegram.org/bot{token}/getFile?file_id={file_id}");
    let f_res: TelegramResponse<FileResult> = client.get(&file_url).send()?.json()?;
    let path = f_res
        .result
        .and_then(|r| r.file_path)
        .context("File path missing in Telegram response")?;

    let dl_url = format!("https://api.telegram.org/file/bot{token}/{path}");
    let bytes = client.get(&dl_url).send()?.bytes()?.to_vec();
    Ok((path, bytes))
}
