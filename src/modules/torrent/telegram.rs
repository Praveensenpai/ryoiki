use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

use crate::runner::Runner;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
    pub qbittorrent_url: String,
}

impl TelegramConfig {
    pub fn config_path() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        Path::new(&home).join(".config/qbittorrent/telegram.json")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        let data = fs::read_to_string(&path)
            .with_context(|| format!("Config file not found at {}", path.display()))?;
        serde_json::from_str(&data).context("Failed to parse telegram.json")
    }

    pub fn save(&self, config_dir: &Path) -> Result<()> {
        let path = config_dir.join("telegram.json");
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&path, data)?;
        Ok(())
    }
}

pub fn prompt_telegram_config(
    _runner: &mut Runner,
    non_interactive: bool,
) -> Result<Option<TelegramConfig>> {
    if non_interactive {
        return Ok(TelegramConfig::load().ok());
    }

    print!("  Configure Telegram bot & alerts? [y/N]: ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;

    if !answer.trim().eq_ignore_ascii_case("y") {
        return Ok(TelegramConfig::load().ok());
    }

    print!("  Enter Telegram Bot Token: ");
    io::stdout().flush()?;
    let mut token = String::new();
    io::stdin().read_line(&mut token)?;
    let bot_token = token.trim().to_string();

    print!("  Enter Telegram Chat ID: ");
    io::stdout().flush()?;
    let mut chat = String::new();
    io::stdin().read_line(&mut chat)?;
    let chat_id = chat.trim().to_string();

    if bot_token.is_empty() || chat_id.is_empty() {
        println!("  ⚠️ Telegram token or chat ID is empty; skipping.");
        return Ok(None);
    }

    let config = TelegramConfig {
        bot_token,
        chat_id,
        qbittorrent_url: "http://localhost:6881".to_string(),
    };

    let test_msg = "🌊 <b>領域 RYOIKI</b> • <i>qBittorrent</i>\n━━━━━━━━━━━━━━━━━━━━━━━\n⚡ <b>Pure-Rust 2-Way Bot Active</b>\n\nType /help to see commands or paste a magnet link!";
    let client = Client::new();
    let _ = client
        .post(format!(
            "https://api.telegram.org/bot{}/sendMessage",
            config.bot_token
        ))
        .form(&[
            ("chat_id", config.chat_id.as_str()),
            ("parse_mode", "HTML"),
            ("text", test_msg),
        ])
        .send();

    println!("  ✔ Sent Telegram test notification");
    Ok(Some(config))
}

pub fn install_bot_service(home: &str) -> Result<()> {
    let service_dir = Path::new(home).join(".config/systemd/user");
    fs::create_dir_all(&service_dir)?;

    let service_file = service_dir.join("ryoiki-bot.service");
    let content = format!(
        "[Unit]\n\
        Description=Ryoiki Pure-Rust 2-Way Telegram Bot for qBittorrent\n\
        After=network.target\n\n\
        [Service]\n\
        Type=simple\n\
        ExecStart={home}/.local/bin/ryoiki bot\n\
        Restart=always\n\
        RestartSec=5\n\n\
        [Install]\n\
        WantedBy=default.target\n"
    );

    fs::write(&service_file, content)?;
    let _ = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output();
    let _ = Command::new("systemctl")
        .args(["--user", "enable", "--now", "ryoiki-bot.service"])
        .output();

    println!("  ✔ Installed and started ryoiki-bot systemd service");
    Ok(())
}

pub fn configure_autorun(lines: &mut Vec<String>, enabled: bool) {
    if !enabled {
        return;
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let bin = format!("{home}/.local/bin/ryoiki");

    let autorun_entries = [
        "enabled=true".to_string(),
        format!("program={bin} notify \"completed\" \"%I\""),
        "OnTorrentAdded\\Enabled=true".to_string(),
        format!("OnTorrentAdded\\Program={bin} notify \"started\" \"%I\""),
    ];

    lines.retain(|l| {
        !l.starts_with("enabled=")
            && !l.starts_with("program=")
            && !l.starts_with("OnTorrentAdded\\")
    });

    let refs: Vec<&str> = autorun_entries.iter().map(String::as_str).collect();
    super::insert_into_section(lines, "[AutoRun]", &refs);
}
