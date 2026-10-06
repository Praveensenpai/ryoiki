use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::fmt::Write as _;
use std::time::Duration;

#[must_use]
pub fn escape_html(input: &str) -> String {
    tayori::infra::telegram::escape_html(input)
}

/// Telegram's hard limit on message text length.
const TELEGRAM_MAX_CHARS: usize = 4096;

/// Renders the current wall-clock time as a human-readable IST stamp.
///
/// IST is a fixed UTC+05:30 offset with no daylight saving, so shifting the
/// epoch and formatting as UTC yields the correct local time without touching
/// the host timezone.
#[must_use]
pub fn ist_timestamp() -> String {
    const IST_OFFSET_SECS: libc::time_t = 19_800;
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let shifted = now + IST_OFFSET_SECS;
        let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
        if libc::gmtime_r(&raw const shifted, tm.as_mut_ptr()).is_null() {
            return "IST".to_string();
        }
        let tm = tm.assume_init();
        let mut buf = [0u8; 64];
        let len = libc::strftime(
            buf.as_mut_ptr().cast(),
            buf.len(),
            c"%a, %d %b %Y %H:%M:%S IST".as_ptr(),
            &raw const tm,
        );
        String::from_utf8_lossy(&buf[..len]).to_string()
    }
}

/// Appends an IST timestamp footer and caps the message to Telegram's limit.
///
/// Truncation cuts on a line boundary so no HTML tag is split in half.
#[must_use]
pub fn finalize_telegram_message(text: &str) -> String {
    let footer = format!("\n\n🕒 <i>{}</i>", ist_timestamp());
    let full = format!("{text}{footer}");
    if full.chars().count() <= TELEGRAM_MAX_CHARS {
        return full;
    }

    let notice = "\n<i>…truncated</i>";
    let reserve = notice.chars().count() + footer.chars().count();
    let keep = TELEGRAM_MAX_CHARS.saturating_sub(reserve);
    let mut body: String = text.chars().take(keep).collect();
    if let Some(idx) = body.rfind('\n') {
        body.truncate(idx);
    }
    format!("{body}{notice}{footer}")
}

pub fn send_telegram_alert(client: &Client, token: &str, chat_id: &str, text: &str) -> Result<()> {
    let text = finalize_telegram_message(text);
    tayori::infra::telegram::send_telegram_raw(client, token, chat_id, &text)
        .context("Failed to dispatch Telegram message via tayori engine")
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

    #[test]
    fn test_ist_timestamp_shape() {
        let stamp = ist_timestamp();
        assert!(stamp.ends_with("IST"), "stamp should end with IST: {stamp}");
        assert!(stamp.contains(char::is_alphabetic));
    }

    #[test]
    fn test_finalize_appends_timestamp_footer() {
        let out = finalize_telegram_message("hello");
        assert!(out.starts_with("hello"));
        assert!(out.contains("IST"));
    }

    #[test]
    fn test_finalize_truncates_long_message() {
        let long = "x".repeat(TELEGRAM_MAX_CHARS * 2);
        let out = finalize_telegram_message(&long);
        assert!(out.chars().count() <= TELEGRAM_MAX_CHARS);
        assert!(out.contains("truncated"));
        assert!(out.contains("IST"));
    }
}
