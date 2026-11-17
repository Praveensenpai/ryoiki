//! Shared Telegram presentation language for RYOIKI.
//!
//! Premium dark minimal, expressed typographically because Telegram message
//! HTML has no background or text-colour control. Every view and alert uses
//! the same compact wordmark, thin rule, aligned monospaced fields, and
//! generous vertical spacing. At most one accent glyph appears per message.

/// Thin divider placed directly under every wordmark.
pub const RULE: &str = "────────────────────";

/// Fixed width of a monospaced field label, keeping values in one column.
pub const LABEL_WIDTH: usize = 11;

/// Wordmark for interactive command views.
#[must_use]
pub fn view(title: &str) -> String {
    format!("<b>RYOIKI</b> <i>/ {title}</i>\n{RULE}")
}

/// Wordmark for autonomous notifications.
#[must_use]
pub fn alert(category: &str) -> String {
    format!("<b>RYOIKI</b> <i>· {category}</i>\n{RULE}")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_uses_wordmark_and_thin_rule() {
        let out = view("STATUS");
        assert!(out.starts_with("<b>RYOIKI</b> <i>/ STATUS</i>\n"));
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
        assert!(alert("Power Event").contains("· Power Event"));
        assert_eq!(accent("✓", "Done"), "✓ <b>Done</b>");
    }
}
