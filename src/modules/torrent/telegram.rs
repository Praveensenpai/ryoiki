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

fn render_notification_script(token: &str, chat: &str, host: &str, ts_ip: &str) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'
FIRST="${{1:-started}}"
[[ "$FIRST" =~ ^completed ]] && EVENT="completed" || EVENT="started"
HASH=$(echo "$*" | grep -oE '\b[0-9a-fA-F]{{40}}\b' | head -n 1 || true)
python3 -c '
import sys, html, time, json, urllib.request, urllib.parse
event, h, tok, cid, srv, tip = sys.argv[1:7]
def fsize(b):
    try:
        v = float(b)
        for u in ["B","KB","MB","GB","TB"]:
            if v < 1024.0: return f"{{v:.2f}} {{u}}"
            v /= 1024.0
        return f"{{v:.2f}} PB"
    except: return "Unknown"
name, sz, cat = "Unknown Torrent", "Unknown", "Default"
if h:
    for _ in range(30):
        try:
            req = urllib.request.Request(f"http://localhost:6881/api/v2/torrents/info?hashes={{h}}")
            with urllib.request.urlopen(req, timeout=5) as r:
                arr = json.loads(r.read().decode())
                if arr:
                    t = arr[0]
                    name = t.get("name", name)
                    raw_sz = t.get("total_size", 0)
                    cat = t.get("category") or cat
                    if event != "started" or (t.get("has_metadata") and raw_sz > 0):
                        sz = fsize(raw_sz)
                        break
        except: pass
        if event != "started": break
        time.sleep(2)
badge = "📥 <b>DOWNLOAD INITIATED</b>" if event == "started" else "✨ <b>DOWNLOAD COMPLETED</b>"
url = f"http://{{tip}}:6881" if tip else "http://localhost:6881"
text = f"""🌊 <b>領域 RYOIKI</b> • <i>qBittorrent</i>
━━━━━━━━━━━━━━━━━━━━━━━
{{badge}}

📦 <b>File:</b> <code>{{html.escape(name)}}</code>
📊 <b>Size:</b> <code>{{sz}}</code>
🏷 <b>Category:</b> <code>{{html.escape(cat)}}</code>
🖥 <b>Host:</b> <code>{{srv}}</code> ({{tip}})

🌐 <a href=\"{{url}}\">Open WebUI</a> • <i>Tailscale</i>
━━━━━━━━━━━━━━━━━━━━━━━"""
data = urllib.parse.urlencode({{"chat_id": cid, "parse_mode": "HTML", "text": text, "disable_web_page_preview": "true"}}).encode()
try: urllib.request.urlopen(urllib.request.Request(f"https://api.telegram.org/bot{{tok}}/sendMessage", data=data), timeout=10)
except: pass
' "$EVENT" "$HASH" "{token}" "{chat}" "{host}" "{ts_ip}" || true
"#
    )
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
    let script_content =
        render_notification_script(&config.bot_token, &config.chat_id, hostname, tailscale_ip);

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
