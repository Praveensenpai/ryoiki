use crate::notify::config::TelegramConfig;
use crate::settings::Settings;
use crate::tui::system;

use super::{App, InputTarget, ProvisionRow, TextInput, ToastLevel};

impl App {
    /// Opens a text input modal seeded with the current value.
    pub fn open_input(&mut self, target: InputTarget) {
        let config = self.telegram.as_ref();
        let token = config.map_or("not set", |cfg| cfg.bot_token.as_str());
        let chat = config.map_or("not set", |cfg| cfg.chat_id.as_str());
        let server = config
            .and_then(|cfg| cfg.server_name.as_deref())
            .unwrap_or("not set");
        let timezone = config
            .and_then(|cfg| cfg.timezone.as_deref())
            .unwrap_or("auto");
        let thresholds = self
            .settings
            .battery
            .alert_thresholds
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(", ");

        let label = match target {
            InputTarget::BotToken => {
                format!("Telegram bot token (current: {})", mask_secret(token))
            }
            InputTarget::ChatId => {
                format!("Telegram chat id (current: {})", mask_secret(chat))
            }
            InputTarget::ServerName => {
                format!("Server display name (current: {server}; empty to clear)")
            }
            InputTarget::Timezone => {
                format!("IANA timezone (current: {timezone}; empty to clear)")
            }
            InputTarget::AlertThresholds => {
                format!("Battery alert thresholds (current: {thresholds})")
            }
        };
        let value = if target == InputTarget::AlertThresholds {
            thresholds
        } else {
            String::new()
        };

        self.input = Some(TextInput {
            target,
            label,
            value,
        });
    }

    /// Applies the open text input to settings or the Telegram config.
    pub fn submit_input(&mut self) {
        let Some(input) = self.input.take() else {
            return;
        };

        if input.target == InputTarget::AlertThresholds {
            match Settings::parse_thresholds(&input.value) {
                Ok(values) => {
                    self.settings.battery.alert_thresholds = values;
                    self.save_settings();
                }
                Err(err) => self.toast(format!("invalid thresholds: {err}"), ToastLevel::Error),
            }
            return;
        }

        let value = input.value.trim();
        if matches!(input.target, InputTarget::BotToken | InputTarget::ChatId) && value.is_empty() {
            self.toast("unchanged", ToastLevel::Info);
            return;
        }

        let mut config = self.telegram.take().unwrap_or_default();
        match input.target {
            InputTarget::BotToken => config.bot_token = value.to_string(),
            InputTarget::ChatId => config.chat_id = value.to_string(),
            InputTarget::ServerName => {
                config.server_name = (!value.is_empty()).then(|| value.to_string());
            }
            InputTarget::Timezone => {
                config.timezone = (!value.is_empty()).then(|| value.to_string());
            }
            InputTarget::AlertThresholds => {}
        }
        let saved = config.save();
        self.telegram = Some(config);
        match saved {
            Ok(()) => self.toast("notification settings saved", ToastLevel::Success),
            Err(err) => self.toast(format!("save failed: {err}"), ToastLevel::Error),
        }
        if self.telegram_error.is_some() {
            self.telegram_error = None;
        }
    }
}

/// Masks a secret, keeping only the last four characters.
#[must_use]
pub fn mask_secret(value: &str) -> String {
    if value.is_empty() {
        return "not set".to_string();
    }
    if value == "not set" {
        return value.to_string();
    }
    let count = value.chars().count();
    if count <= 6 {
        return "••••".to_string();
    }
    let tail: String = value.chars().skip(count - 4).collect();
    format!("••••{tail}")
}

pub(super) fn settings_warning() -> Option<String> {
    let path = crate::settings::settings_path();
    if !path.exists() {
        return None;
    }
    crate::settings::load()
        .is_err()
        .then(|| format!("{} is invalid; defaults loaded", path.display()))
}

pub(super) fn telegram_state() -> (Option<TelegramConfig>, Option<String>) {
    match TelegramConfig::load() {
        Ok(cfg) => (Some(cfg), None),
        Err(_) => (None, Some("telegram.json not found".to_string())),
    }
}

pub(super) fn provision_rows() -> Vec<ProvisionRow> {
    let completed = crate::state::RunState::load()
        .map(|state| state.completed)
        .unwrap_or_default();
    crate::modules::get_available_modules()
        .into_iter()
        .map(|module| {
            let id = module.id.to_string();
            let installed = completed.contains(&id) || system::module_installed(&id);
            ProvisionRow {
                id,
                title: module.title.to_string(),
                description: module.description.to_string(),
                selected: module.default_enabled,
                installed,
            }
        })
        .collect()
}
