//! Trust gate for messages from users who are not on the allowlist.
//!
//! Strangers may only send `/request`, which forwards an Approve/Deny prompt
//! to the owner. Everything else, including `/start`, is answered with
//! instructions to request access.

use reqwest::blocking::Client;

use super::access::AccessControl;
use super::client::{reply, reply_with_keyboard};
use super::keyboards::access_request_keyboard;
use super::router::extract_command;
use super::types::{Message, User};
use crate::notify::client::escape_html;
use crate::notify::config::TelegramConfig;

/// Message shown to untrusted users who have not requested access yet.
pub(crate) const ACCESS_REQUIRED_TEXT: &str =
    "🔒 <b>Access required.</b>\nSend /request to ask the owner for permission.";

/// What an untrusted user's message should trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UntrustedAction {
    /// `/request`: forward an approval prompt to the owner.
    RequestAccess,
    /// Any other command, including `/start`: explain how to request access.
    Deny,
}

/// Classifies an untrusted user's text into a gate action.
pub(crate) fn classify_untrusted(text: Option<&str>) -> UntrustedAction {
    match text.and_then(extract_command) {
        Some(("/request", _)) => UntrustedAction::RequestAccess,
        _ => UntrustedAction::Deny,
    }
}

/// Answers a stranger: `/request` notifies the owner, anything else is refused.
pub(crate) fn handle_untrusted_message(
    client: &Client,
    config: &TelegramConfig,
    access: &AccessControl,
    msg: &Message,
    user: &User,
) {
    let chat_id = msg.chat.as_ref().map_or(user.id, |c| c.id).to_string();
    let text = msg.text.as_deref().or(msg.caption.as_deref());

    if classify_untrusted(text) == UntrustedAction::Deny {
        let _ = reply(client, &config.bot_token, &chat_id, ACCESS_REQUIRED_TEXT);
        return;
    }

    if access.is_pending(user.id) {
        let _ = reply(
            client,
            &config.bot_token,
            &chat_id,
            "⏳ <b>Request already pending.</b> The owner has not answered yet.",
        );
        return;
    }

    access.mark_pending(user.id);
    let _ = reply(
        client,
        &config.bot_token,
        &chat_id,
        "⏳ <b>Request sent.</b> You will get access once the owner approves.",
    );

    let name = escape_html(&user.display_name());
    let prompt = format!(
        "🔐 <b>Access Request</b>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        User: {name}\n\
        ID: <code>{}</code>\n\n\
        Approve this user?",
        user.id
    );
    let kb = access_request_keyboard(user.id);
    let _ = reply_with_keyboard(client, &config.bot_token, &config.chat_id, &prompt, &kb);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_untrusted_start_and_plain_text_are_denied() {
        assert_eq!(classify_untrusted(Some("/start")), UntrustedAction::Deny);
        assert_eq!(
            classify_untrusted(Some("/start@ryoikiServerBot")),
            UntrustedAction::Deny
        );
        assert_eq!(classify_untrusted(Some("/status")), UntrustedAction::Deny);
        assert_eq!(classify_untrusted(Some("hello")), UntrustedAction::Deny);
        assert_eq!(classify_untrusted(None), UntrustedAction::Deny);
        assert!(ACCESS_REQUIRED_TEXT.contains("/request"));
    }

    #[test]
    fn test_untrusted_request_command() {
        assert_eq!(
            classify_untrusted(Some("/request")),
            UntrustedAction::RequestAccess
        );
        assert_eq!(
            classify_untrusted(Some(" /request@ryoikiServerBot ")),
            UntrustedAction::RequestAccess
        );
    }
}
