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
    let footer = format!("\n\n<i>{}</i>", ist_timestamp());
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
        let label = plain_label(key);
        let _ = writeln!(
            body,
            "{}<code>{val}</code>",
            crate::notify::style::key(&label)
        );
    }

    format!(
        "{}\n\n{}\n\n{}",
        crate::notify::style::alert(category),
        plain_badge(badge),
        body.trim_end()
    )
}

/// Strips legacy leading emoji from alert badges so every card uses the same
/// restrained typographic headline.
fn plain_badge(badge: &str) -> String {
    let trimmed = badge.trim();
    let Some((glyph, rest)) = trimmed.split_once(' ') else {
        return trimmed.to_string();
    };
    if glyph.starts_with('<') || glyph.chars().all(char::is_alphanumeric) {
        return trimmed.to_string();
    }
    rest.trim_start().to_string()
}

/// Removes a legacy leading glyph and trailing colon from a card field label.
fn plain_label(key: &str) -> String {
    let trimmed = key.trim().trim_end_matches(':');
    let Some((glyph, rest)) = trimmed.split_once(' ') else {
        return trimmed.to_string();
    };
    if glyph.chars().all(char::is_alphanumeric) {
        return trimmed.to_string();
    }
    rest.trim_start().to_string()
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
        let card = format_card("Test", "<b>OK</b>", &[("💾 Key:", "Val")]);
        assert!(card.contains("<b>RYOIKI</b> <i>· Test</i>"));
        assert!(card.contains("<b>OK</b>"));
        assert!(card.contains("<code>Key        </code><code>Val</code>"));
        assert!(!card.contains('━'));
        assert!(!card.contains("領域"));
        assert!(!card.contains('💾'));
    }

    #[test]
    fn test_plain_badge_strips_legacy_glyph() {
        assert_eq!(plain_badge("✨ <b>ONLINE</b>"), "<b>ONLINE</b>");
        assert_eq!(plain_badge("<b>ONLINE</b>"), "<b>ONLINE</b>");
        assert_eq!(plain_badge("NEW SESSION"), "NEW SESSION");
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
