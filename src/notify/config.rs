use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

fn default_qbittorrent_url() -> String {
    "http://localhost:6881".to_string()
}

fn default_api_port() -> u16 {
    9119
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
    #[serde(default = "default_qbittorrent_url")]
    pub qbittorrent_url: String,
    #[serde(default)]
    pub server_name: Option<String>,
    #[serde(default = "default_api_port")]
    pub api_port: u16,
}

impl TelegramConfig {
    #[must_use]
    pub fn primary_config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        Path::new(&home).join(".config/ryoiki/telegram.json")
    }

    #[must_use]
    pub fn legacy_config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        Path::new(&home).join(".config/qbittorrent/telegram.json")
    }

    pub fn load() -> Result<Self> {
        let primary = Self::primary_config_path();
        if primary.exists() {
            let data = fs::read_to_string(&primary)
                .with_context(|| format!("Failed to read {}", primary.display()))?;
            return serde_json::from_str(&data)
                .with_context(|| format!("Failed to parse {}", primary.display()));
        }

        let legacy = Self::legacy_config_path();
        if legacy.exists() {
            let data = fs::read_to_string(&legacy)
                .with_context(|| format!("Failed to read {}", legacy.display()))?;
            return serde_json::from_str(&data)
                .with_context(|| format!("Failed to parse {}", legacy.display()));
        }

        anyhow::bail!(
            "Telegram config not found. Expected at {} or {}",
            primary.display(),
            legacy.display()
        )
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::primary_config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&path, data)?;
        Ok(())
    }

    pub fn save_to(&self, config_dir: &Path) -> Result<()> {
        fs::create_dir_all(config_dir)?;
        let path = config_dir.join("telegram.json");
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&path, data)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_config_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"{"bot_token": "123:ABC", "chat_id": "999"}"#;
        let cfg: TelegramConfig = serde_json::from_str(json)?;
        assert_eq!(cfg.bot_token, "123:ABC");
        assert_eq!(cfg.chat_id, "999");
        assert_eq!(cfg.qbittorrent_url, "http://localhost:6881");
        assert_eq!(cfg.api_port, 9119);
        assert!(cfg.server_name.is_none());
        Ok(())
    }
}
