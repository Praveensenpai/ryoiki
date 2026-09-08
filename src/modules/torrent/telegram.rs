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

    let test_msg = "⚡ <b>領域 (Ryoiki)</b>: Telegram alerts configured for qBittorrent!";
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

pub fn install_notification_script(config_dir: &Path, config: &TelegramConfig) -> Result<()> {
    let scripts_dir = config_dir.join("scripts");
    fs::create_dir_all(&scripts_dir)
        .with_context(|| format!("Failed to create scripts directory {}", scripts_dir.display()))?;

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

format_size() {{
    local bytes="$1"
    if [ "$bytes" -ge 1073741824 ] 2>/dev/null; then
        python3 -c "print(f'{{$bytes / 1073741824:.2f}} GB')" 2>/dev/null || echo "${{bytes}} B"
    elif [ "$bytes" -ge 1048576 ] 2>/dev/null; then
        python3 -c "print(f'{{$bytes / 1048576:.2f}} MB')" 2>/dev/null || echo "${{bytes}} B"
    elif [ "$bytes" -ge 1024 ] 2>/dev/null; then
        python3 -c "print(f'{{$bytes / 1024:.2f}} KB')" 2>/dev/null || echo "${{bytes}} B"
    else
        echo "${{bytes}} B"
    fi
}}

SIZE_FORMATTED=$(format_size "$TORRENT_SIZE_BYTES")
HOSTNAME=$(hostname 2>/dev/null || echo "mochi")

case "$EVENT" in
    started)
        TITLE="📥 <b>Torrent Started</b>"
        ;;
    completed)
        TITLE="✅ <b>Torrent Completed</b>"
        ;;
    *)
        TITLE="ℹ️ <b>Torrent Event: ${{EVENT}}</b>"
        ;;
esac

TEXT="${{TITLE}}
━━━━━━━━━━━━━━━━━━━━━
<b>Name:</b> <code>${{TORRENT_NAME}}</code>
<b>Size:</b> ${{SIZE_FORMATTED}}
<b>Category:</b> ${{CATEGORY}}
<b>Server:</b> ${{HOSTNAME}}"

curl -s -X POST "https://api.telegram.org/bot${{BOT_TOKEN}}/sendMessage" \
    --data-urlencode "chat_id=${{CHAT_ID}}" \
    --data-urlencode "parse_mode=HTML" \
    --data-urlencode "text=${{TEXT}}" \
    --max-time 10 >/dev/null 2>&1 || true
"#,
        bot_token = config.bot_token,
        chat_id = config.chat_id,
    );

    fs::write(&script_path, script_content)
        .with_context(|| format!("Failed to write notification script {}", script_path.display()))?;

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
