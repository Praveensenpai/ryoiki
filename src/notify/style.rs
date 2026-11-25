//! Shared Telegram presentation language for RYOIKI.
//!
//! Lively branded cards with blockquote containers, health-aware status dots,
//! and consistent emoji accents. Every view and alert uses the 領域 wordmark,
//! thin rule, aligned monospaced fields, and status-coloured indicators.

/// Thin divider placed directly under every wordmark.
pub const RULE: &str = "────────────────────";

/// Fixed width of a monospaced field label, keeping values in one column.
pub const LABEL_WIDTH: usize = 11;

/// Wordmark for interactive command views.
#[must_use]
pub fn view(title: &str) -> String {
    format!("🌸 <b>領域 RYOIKI</b> · <i>{title}</i>\n{RULE}")
}

/// Wordmark for autonomous notifications.
#[must_use]
pub fn alert(category: &str) -> String {
    format!("🌸 <b>RYOIKI</b> · <i>{category}</i>\n{RULE}")
}

/// Quiet uppercase section eyebrow.
#[must_use]
pub fn eyebrow(label: &str) -> String {
    format!("<i>{label}</i>")
}

/// Monospaced, fixed-width field label.
#[must_use]
pub fn key(label: &str) -> String {
    format!("<code>{label:<LABEL_WIDTH$}</code>")
}

/// Column-aligned field row. The label is monospaced; the value may contain
/// trusted Telegram HTML.
#[must_use]
pub fn row(label: &str, value: &str) -> String {
    format!("{}{value}", key(label))
}

/// The single accent slot in a message, used for state changes and warnings.
#[must_use]
pub fn accent(glyph: &str, text: &str) -> String {
    format!("{glyph} <b>{text}</b>")
}

/// Wraps content in a Telegram blockquote card (tinted left-bar container).
#[must_use]
pub fn card(content: &str) -> String {
    format!("<blockquote>{content}</blockquote>")
}

/// Wraps content in a collapsible expandable blockquote (collapsed by default).
#[must_use]
pub fn drawer(content: &str) -> String {
    format!("<blockquote expandable>{content}</blockquote>")
}

/// Returns a coloured dot based on a usage percentage.
/// Green below 65%, yellow 65-84%, red at 85%+.
#[must_use]
pub fn status_dot(pct: u8) -> &'static str {
    if pct >= 85 {
        "🔴"
    } else if pct >= 65 {
        "🟡"
    } else {
        "🟢"
    }
}

/// Returns a coloured dot based on a systemd/docker service state string.
#[must_use]
pub fn service_dot(state: &str) -> &'static str {
    match state {
        "active" | "running" => "🟢",
        "failed" | "error" => "🔴",
        "activating" | "reloading" => "🟡",
        _ => "⚪",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_uses_wordmark_and_thin_rule() {
        let out = view("STATUS");
        assert!(out.contains("RYOIKI"));
        assert!(out.contains("STATUS"));
        assert!(out.ends_with(RULE));
        assert!(!out.contains('━'));
    }

    #[test]
    fn test_row_pads_label_for_alignment() {
        let out = row("HOST", "node-01");
        assert!(out.starts_with("<code>HOST       </code>"));
        assert!(out.ends_with("node-01"));
    }

    #[test]
    fn test_alert_and_accent() {
        let out = alert("Power Event");
        assert!(out.contains("RYOIKI"));
        assert!(out.contains("Power Event"));
        assert_eq!(accent("✓", "Done"), "✓ <b>Done</b>");
    }

    #[test]
    fn test_card_and_drawer_wrap_content() {
        assert_eq!(card("hello"), "<blockquote>hello</blockquote>");
        assert_eq!(drawer("hello"), "<blockquote expandable>hello</blockquote>");
    }

    #[test]
    fn test_status_dot_thresholds() {
        assert_eq!(status_dot(0), "🟢");
        assert_eq!(status_dot(64), "🟢");
        assert_eq!(status_dot(65), "🟡");
        assert_eq!(status_dot(84), "🟡");
        assert_eq!(status_dot(85), "🔴");
        assert_eq!(status_dot(100), "🔴");
    }

    #[test]
    fn test_service_dot_states() {
        assert_eq!(service_dot("active"), "🟢");
        assert_eq!(service_dot("running"), "🟢");
        assert_eq!(service_dot("failed"), "🔴");
        assert_eq!(service_dot("activating"), "🟡");
        assert_eq!(service_dot("inactive"), "⚪");
    }
}
