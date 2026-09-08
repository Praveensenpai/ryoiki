use anyhow::{Context, Result};
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use crate::runner::Runner;

pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
}

pub fn prompt_telegram_config(
    _runner: &mut Runner,
    non_interactive: bool,
) -> Result<Option<TelegramConfig>> {
    if non_interactive {
        return Ok(None);
    }

    print!("  Configure Telegram notifications? [y/N]: ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;

    if !answer.trim().eq_ignore_ascii_case("y") {
        return Ok(None);
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
        println!("  ⚠️ Telegram token or chat ID is empty; skipping Telegram setup.");
        return Ok(None);
    }

    let test_msg = "🌊 <b>領域 RYOIKI</b> • <i>qBittorrent</i>\n━━━━━━━━━━━━━━━━━━━━━━━\n⚡ <b>Alerts Configured Successfully</b>\n\nNotifications for downloads will arrive here.";
    let _ = Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            &format!("https://api.telegram.org/bot{bot_token}/sendMessage"),
            "--data-urlencode",
            &format!("chat_id={chat_id}"),
            "--data-urlencode",
            "parse_mode=HTML",
            "--data-urlencode",
            &format!("text={test_msg}"),
            "--max-time",
            "10",
        ])
        .output();

    println!("  ✔ Sent Telegram test notification");
    Ok(Some(TelegramConfig { bot_token, chat_id }))
}

pub fn install_notification_script(
    config_dir: &Path,
    config: &TelegramConfig,
    (hostname, tailscale_ip): (&str, &str),
) -> Result<()> {
    let scripts_dir = config_dir.join("scripts");
    fs::create_dir_all(&scripts_dir).with_context(|| {
        format!(
            "Failed to create scripts directory {}",
            scripts_dir.display()
        )
    })?;

    let script_path = scripts_dir.join("telegram_notify.sh");
    let script_content = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

EVENT="${{1:-unknown}}"
TORRENT_NAME="${{2:-Unknown Torrent}}"
TORRENT_SIZE_BYTES="${{3:-0}}"
CATEGORY="${{4:-None}}"
INFO_HASH="${{5:-}}"

BOT_TOKEN="{bot_token}"
CHAT_ID="{chat_id}"
SERVER_NAME="{hostname}"
TAILSCALE_IP="{tailscale_ip}"

python3 -c '
import sys, html, urllib.request, urllib.parse

event = sys.argv[1]
name = sys.argv[2]
raw_size = sys.argv[3]
category = sys.argv[4] if len(sys.argv) > 4 and sys.argv[4] else "Default"
token = sys.argv[5]
chat_id = sys.argv[6]
host = sys.argv[7]
ts_ip = sys.argv[8]

def format_size(b_str):
    try:
        b = float(b_str)
        for unit in ["B", "KB", "MB", "GB", "TB"]:
            if b < 1024.0:
                return f"{{b:.2f}} {{unit}}"
            b /= 1024.0
        return f"{{b:.2f}} PB"
    except Exception:
        return "Unknown"

size_str = format_size(raw_size)
esc_name = html.escape(name)
esc_cat = html.escape(category)

if event == "started":
    badge = "📥 <b>DOWNLOAD INITIATED</b>"
elif event == "completed":
    badge = "✨ <b>DOWNLOAD COMPLETED</b>"
else:
    badge = f"ℹ️ <b>EVENT: {{html.escape(event).upper()}}</b>"

webui_url = f"http://{{ts_ip}}:6881" if ts_ip else "http://localhost:6881"

text = f"""🌊 <b>領域 RYOIKI</b> • <i>qBittorrent</i>
━━━━━━━━━━━━━━━━━━━━━━━
{{badge}}

📦 <b>File:</b> <code>{{esc_name}}</code>
📊 <b>Size:</b> <code>{{size_str}}</code>
🏷 <b>Category:</b> <code>{{esc_cat}}</code>
🖥 <b>Host:</b> <code>{{host}}</code> ({{ts_ip}})

🌐 <a href=\"{{webui_url}}\">Open WebUI</a> • <i>Tailscale</i>
━━━━━━━━━━━━━━━━━━━━━━━"""

data = urllib.parse.urlencode({{
    "chat_id": chat_id,
    "parse_mode": "HTML",
    "text": text,
    "disable_web_page_preview": "true",
}}).encode("utf-8")

req = urllib.request.Request(f"https://api.telegram.org/bot{{token}}/sendMessage", data=data)
try:
    urllib.request.urlopen(req, timeout=10)
except Exception:
    pass
' "$EVENT" "$TORRENT_NAME" "$TORRENT_SIZE_BYTES" "$CATEGORY" "$BOT_TOKEN" "$CHAT_ID" "$SERVER_NAME" "$TAILSCALE_IP" || true
"#,
        bot_token = config.bot_token,
        chat_id = config.chat_id,
        hostname = hostname,
        tailscale_ip = tailscale_ip,
    );

    fs::write(&script_path, script_content).with_context(|| {
        format!(
            "Failed to write notification script {}",
            script_path.display()
        )
    })?;

    let mut perms = fs::metadata(&script_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms)?;

    Ok(())
}

pub fn configure_autorun(lines: &mut Vec<String>, script_installed: bool) {
    if !script_installed {
        return;
    }

    let autorun_entries = [
        "enabled=true",
        "program=/config/scripts/telegram_notify.sh \"completed\" \"%N\" \"%Z\" \"%L\" \"%I\"",
        "OnTorrentAdded\\Enabled=true",
        "OnTorrentAdded\\Program=/config/scripts/telegram_notify.sh \"started\" \"%N\" \"%Z\" \"%L\" \"%I\"",
    ];

    lines.retain(|l| {
        !l.starts_with("enabled=")
            && !l.starts_with("program=")
            && !l.starts_with("OnTorrentAdded\\")
    });

    super::insert_into_section(lines, "[AutoRun]", &autorun_entries);
}
