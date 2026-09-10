use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::fmt::Write as _;
use std::time::Duration;

#[must_use]
pub fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn send_telegram_alert(client: &Client, token: &str, chat_id: &str, text: &str) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");
    let params = [
        ("chat_id", chat_id),
        ("parse_mode", "HTML"),
        ("text", text),
        ("disable_web_page_preview", "true"),
    ];

    let resp = client
        .post(&url)
        .form(&params)
        .send()
        .context("Failed to send HTTP request to Telegram API")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        anyhow::bail!("Telegram sendMessage failed with HTTP {status}: {body}");
    }
    Ok(())
}

pub fn send_alert(token: &str, chat_id: &str, text: &str) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .context("Failed to build HTTP client for Telegram alert")?;

    send_telegram_alert(&client, token, chat_id, text)
}

#[must_use]
pub fn format_card(category: &str, badge: &str, fields: &[(&str, &str)]) -> String {
    let mut body = String::new();
    for (key, val) in fields {
        let _ = writeln!(body, "{key} <code>{val}</code>");
    }

    format!(
        "🌊 <b>領域 RYOIKI</b> • <i>{category}</i>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        {badge}\n\n\
        {body}\
        ━━━━━━━━━━━━━━━━━━━━━━━"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_html() {
        assert_eq!(
            escape_html("<script>alert(\"x & y\")</script>"),
            "&lt;script&gt;alert(&quot;x &amp; y&quot;)&lt;/script&gt;"
        );
    }

    #[test]
    fn test_format_card() {
        let card = format_card("Test", "✨ OK", &[("Key:", "Val")]);
        assert!(card.contains("領域 RYOIKI"));
        assert!(card.contains("✨ OK"));
        assert!(card.contains("Key: <code>Val</code>"));
    }
}
