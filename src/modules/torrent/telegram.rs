use anyhow::Result;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

pub use crate::notify::TelegramConfig;
use crate::runner::Runner;

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

    print!("  Enter Gemini API Key for AI media classification [skip]: ");
    io::stdout().flush()?;
    let mut key_input = String::new();
    io::stdin().read_line(&mut key_input)?;
    let gemini_api_key = {
        let k = key_input.trim();
        if k.is_empty() {
            None
        } else {
            Some(k.to_string())
        }
    };

    let config = TelegramConfig {
        bot_token,
        chat_id,
        qbittorrent_url: "http://localhost:6881".to_string(),
        server_name: None,
        api_port: 9119,
        gemini_api_key,
    };

    let test_msg = "🌊 <b>領域 RYOIKI</b> • <i>qBittorrent</i>\n━━━━━━━━━━━━━━━━━━━━━━━\n⚡ <b>Pure-Rust 2-Way Bot Active</b>\n\nType /help to see commands or paste a magnet link!";
    let _ = crate::notify::client::send_alert(&config.bot_token, &config.chat_id, test_msg);

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
