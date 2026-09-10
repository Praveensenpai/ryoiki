use anyhow::Result;
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

    #[must_use]
    pub fn candidate_paths() -> Vec<PathBuf> {
        let mut paths = vec![
            Self::primary_config_path(),
            Self::legacy_config_path(),
            PathBuf::from("/etc/ryoiki/telegram.json"),
        ];

        if let Ok(entries) = fs::read_dir("/home") {
            for entry in entries.flatten() {
                let user_dir = entry.path();
                paths.push(user_dir.join(".config/ryoiki/telegram.json"));
                paths.push(user_dir.join(".config/qbittorrent/telegram.json"));
            }
        }
        paths
    }

    pub fn load() -> Result<Self> {
        for path in Self::candidate_paths() {
            if path.exists() {
                if let Ok(data) = fs::read_to_string(&path) {
                    if let Ok(cfg) = serde_json::from_str::<Self>(&data) {
                        return Ok(cfg);
                    }
                }
            }
        }

        anyhow::bail!(
            "Telegram config not found in candidate paths: {:?}",
            Self::candidate_paths()
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
