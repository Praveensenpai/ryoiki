use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct TelegramResponse<T> {
    pub result: Option<T>,
}

#[derive(Debug, Deserialize)]
pub struct Update {
    #[serde(rename = "update_id")]
    pub id: i64,
    pub message: Option<Message>,
    pub callback_query: Option<CallbackQuery>,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    #[serde(rename = "message_id")]
    pub id: i64,
    pub from: Option<User>,
    pub text: Option<String>,
    pub document: Option<Document>,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub id: String,
    pub from: User,
    pub message: Option<Message>,
    pub data: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct Document {
    pub file_id: String,
    pub file_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FileResult {
    pub file_path: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct InlineKeyboardMarkup {
    pub inline_keyboard: Vec<Vec<InlineKeyboardButton>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct InlineKeyboardButton {
    pub text: String,
    pub callback_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl InlineKeyboardButton {
    #[must_use]
    pub fn callback(text: &str, data: &str) -> Self {
        Self {
            text: text.to_string(),
            callback_data: Some(data.to_string()),
            url: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_update_with_callback() {
        let json_data = r#"{
            "update_id": 99999,
            "callback_query": {
                "id": "cb123",
                "from": { "id": 123456 },
                "data": "cb:status"
            }
        }"#;

        let Ok(u) = serde_json::from_str::<Update>(json_data) else {
            panic!("Failed to deserialize update");
        };
        assert_eq!(u.id, 99_999);
        let Some(cb) = u.callback_query else {
            panic!("Callback query missing");
        };
        assert_eq!(cb.id, "cb123");
        assert_eq!(cb.from.id, 123_456);
        assert_eq!(cb.data.as_deref(), Some("cb:status"));
    }

    #[test]
    fn test_deserialize_update_with_message() {
        let json_data = r#"{
            "update_id": 88888,
            "message": {
                "message_id": 42,
                "from": { "id": 123456 },
                "text": "/status"
            }
        }"#;

        let Ok(u) = serde_json::from_str::<Update>(json_data) else {
            panic!("Failed to deserialize update");
        };
        assert_eq!(u.id, 88_888);
        let Some(msg) = u.message else {
            panic!("Message missing");
        };
        assert_eq!(msg.id, 42);
        assert_eq!(msg.text.as_deref(), Some("/status"));
    }
}
