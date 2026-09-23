use anyhow::Result;
use std::fs;
use std::mem::MaybeUninit;
use std::process::Command;
use std::time::Duration;

use super::client::{escape_html, format_card, send_alert};
use super::config::TelegramConfig;

pub fn send_boot_notification(config: &TelegramConfig) -> Result<()> {
    let mut last_err = None;
    for attempt in 1..=5 {
        let card = build_boot_card(config);
        match send_alert(&config.bot_token, &config.chat_id, &card) {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                if attempt < 5 {
                    std::thread::sleep(Duration::from_secs(4));
                }
            }
        }
    }
    last_err.map_or(Ok(()), Err)
}

fn build_boot_card(config: &TelegramConfig) -> String {
    let host = get_hostname(config);
    let uptime = get_uptime();
    let pub_ip = get_public_ip();
    let ts_ip = get_tailscale_ip();
    let kernel = get_kernel();

    let mem_str = get_memory_stats().map_or_else(|| "N/A".to_string(), |(u, t)| format_usage(u, t));
    let disk_str = get_disk_stats().map_or_else(|| "N/A".to_string(), |(u, t)| format_usage(u, t));

    let fields = [
        ("🚀 Host:", host.as_str()),
        ("⏱ Uptime:", uptime.as_str()),
        ("🌐 Public IP:", pub_ip.as_str()),
        (
            "🦎 Tailscale:",
            if ts_ip.is_empty() {
                "none"
            } else {
                ts_ip.as_str()
            },
        ),
        ("🧠 Memory:", mem_str.as_str()),
        ("💾 Root Disk:", disk_str.as_str()),
        ("🐧 Kernel:", kernel.as_str()),
    ];

    format_card("System Boot", "✨ <b>SYSTEM ONLINE</b>", &fields)
}

pub fn send_login_notification(
    config: &TelegramConfig,
    user: Option<&str>,
    ip: Option<&str>,
    service: Option<&str>,
    tty: Option<&str>,
) -> Result<()> {
    if std::env::var("PAM_TYPE").as_deref() == Ok("close_session") {
        return Ok(());
    }

    let resolved_user = resolve_user(user);
    let resolved_ip = resolve_ip(ip);

    if super::session::is_session_suppressed(&resolved_ip, config.session_cooldown_mins) {
        return Ok(());
    }

    let resolved_srv = service.unwrap_or("sshd");
    let resolved_tty = tty.unwrap_or("pts/0");
    let host = get_hostname(config);
    let srv_label = format!("{resolved_srv} ({resolved_tty})");

    let fields = [
        ("👤 User:", resolved_user.as_str()),
        ("🌐 Client IP:", resolved_ip.as_str()),
        ("🖥 Host:", host.as_str()),
        ("🛠 Service:", srv_label.as_str()),
    ];

    let card = format_card("Security Alert", "🔐 <b>NEW SESSION OPENED</b>", &fields);
    send_alert(&config.bot_token, &config.chat_id, &card)?;
    super::session::record_session_login(&resolved_ip);
    Ok(())
}

pub fn send_custom_notification(
    config: &TelegramConfig,
    message: &str,
    title: Option<&str>,
    level: &str,
) -> Result<()> {
    let badge = match level.to_lowercase().as_str() {
        "success" => "✨ <b>SUCCESS</b>",
        "warn" | "warning" => "⚠️ <b>WARNING</b>",
        "error" | "danger" => "🚨 <b>ALERT</b>",
        _ => "ℹ️ <b>NOTIFICATION</b>",
    };

    let category = title.unwrap_or("System Event");
    let escaped = escape_html(message);
    let card = format!(
        "🌊 <b>領域 RYOIKI</b> • <i>{category}</i>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        {badge}\n\n\
        {escaped}\n\
        ━━━━━━━━━━━━━━━━━━━━━━━"
    );

    send_alert(&config.bot_token, &config.chat_id, &card)
}

fn resolve_user(arg: Option<&str>) -> String {
    if let Some(u) = arg {
        if !u.is_empty() {
            return u.to_string();
        }
    }
    std::env::var("PAM_USER")
        .or_else(|_| std::env::var("USER"))
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

fn resolve_ip(arg: Option<&str>) -> String {
    if let Some(ip) = arg {
        if !ip.is_empty() {
            return ip.to_string();
        }
    }
    if let Ok(ip) = std::env::var("PAM_RHOST") {
        if !ip.is_empty() {
            return ip;
        }
    }
    if let Ok(client) = std::env::var("SSH_CLIENT").or_else(|_| std::env::var("SSH_CONNECTION")) {
        if let Some(first) = client.split_whitespace().next() {
            return first.to_string();
        }
    }
    "Local Console".to_string()
}

fn get_hostname(config: &TelegramConfig) -> String {
    if let Some(name) = &config.server_name {
        return name.clone();
    }
    Command::new("hostname").output().map_or_else(
        |_| "ryoiki-server".to_string(),
        |o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                "ryoiki-server".to_string()
            } else {
                s
            }
        },
    )
}

fn get_public_ip() -> String {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build();
    if let Ok(c) = client {
        if let Ok(resp) = c.get("https://api64.ipify.org").send() {
            if let Ok(ip) = resp.text() {
                let trimmed = ip.trim();
                if !trimmed.is_empty() && trimmed.len() < 45 {
                    return trimmed.to_string();
                }
            }
        }
    }
    "unavailable".to_string()
}

fn get_tailscale_ip() -> String {
    Command::new("tailscale")
        .args(["ip", "-4"])
        .output()
        .map_or_else(
            |_| String::new(),
            |o| String::from_utf8_lossy(&o.stdout).trim().to_string(),
        )
}

fn get_kernel() -> String {
    fs::read_to_string("/proc/sys/kernel/osrelease")
        .map_or_else(|_| "Linux".to_string(), |s| s.trim().to_string())
}

fn get_uptime() -> String {
    if let Ok(content) = fs::read_to_string("/proc/uptime") {
        if let Some(sec_str) = content.split_whitespace().next() {
            let int_part = sec_str.split('.').next().unwrap_or(sec_str);
            if let Ok(s) = int_part.parse::<u64>() {
                let days = s / 86400;
                let hours = (s % 86400) / 3600;
                let mins = (s % 3600) / 60;
                if days > 0 {
                    return format!("{days}d {hours}h {mins}m");
                }
                if hours > 0 {
                    return format!("{hours}h {mins}m");
                }
                return format!("{mins}m {}s", s % 60);
            }
        }
    }
    "unknown".to_string()
}

fn get_memory_stats() -> Option<(u64, u64)> {
    let content = fs::read_to_string("/proc/meminfo").ok()?;
    let mut total_kb: Option<u64> = None;
    let mut avail_kb: Option<u64> = None;
    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            total_kb = line.split_whitespace().nth(1).and_then(|v| v.parse().ok());
        } else if line.starts_with("MemAvailable:") {
            avail_kb = line.split_whitespace().nth(1).and_then(|v| v.parse().ok());
        }
    }
    let total = total_kb?.saturating_mul(1024);
    let avail = avail_kb?.saturating_mul(1024);
    let used = total.saturating_sub(avail);
    Some((used, total))
}

fn get_disk_stats() -> Option<(u64, u64)> {
    let mut stat = MaybeUninit::<libc::statvfs>::uninit();
    let path = std::ffi::CString::new("/").ok()?;
    let res = unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) };
    if res != 0 {
        return None;
    }
    let stat = unsafe { stat.assume_init() };
    let total = stat.f_blocks.saturating_mul(stat.f_frsize);
    let avail = stat.f_bavail.saturating_mul(stat.f_frsize);
    let used = total.saturating_sub(avail);
    Some((used, total))
}

fn format_usage(used: u64, total: u64) -> String {
    if total == 0 {
        return "N/A".to_string();
    }
    let one_gb = 1024 * 1024 * 1024;
    let u_gb = used / one_gb;
    let u_dec = (used % one_gb) * 10 / one_gb;
    let t_gb = total / one_gb;
    let t_dec = (total % one_gb) * 10 / one_gb;
    let pct = (used * 100) / total;
    let pct_dec = ((used * 1000) / total) % 10;
    format!("{u_gb}.{u_dec} / {t_gb}.{t_dec} GB ({pct}.{pct_dec}%)")
}

pub fn send_audio_strip_notification(
    config: &TelegramConfig,
    args: &crate::notify::AudioStripArgs,
) -> Result<()> {
    let host = get_hostname(config);
    let ts_ip = get_tailscale_ip();
    let host_info = if ts_ip.is_empty() {
        host
    } else {
        format!("{host} ({ts_ip})")
    };

    let title_esc = escape_html(&args.title);
    let origin_esc = escape_html(&args.origin);
    let kept_esc = escape_html(&args.kept);
    let stripped_esc = escape_html(&args.stripped);
    let prev_esc = escape_html(&args.prev_size);
    let new_esc = escape_html(&args.new_size);
    let rec_esc = escape_html(&args.reclaimed);

    let is_multi_preserved = args.stripped.to_lowercase().contains("none")
        || args.reclaimed.to_lowercase().contains("multi");

    let card = if is_multi_preserved {
        let fields = [
            ("🎬 Title:", title_esc.as_str()),
            ("🧠 Origin:", origin_esc.as_str()),
            ("🛡 Kept Tracks:", kept_esc.as_str()),
            ("📦 Size:", prev_esc.as_str()),
            ("ℹ Status:", "Preserved Multi (Low confidence / Uncertain)"),
            ("🖥 Host:", host_info.as_str()),
        ];
        format_card("DubStrip", "🛡️ <b>AUDIO PRESERVED AS MULTI</b>", &fields)
    } else {
        let fields = [
            ("🎬 Title:", title_esc.as_str()),
            ("🧠 Origin:", origin_esc.as_str()),
            ("🛡 Kept:", kept_esc.as_str()),
            ("🗑 Stripped:", stripped_esc.as_str()),
            ("📦 Previous:", prev_esc.as_str()),
            ("📦 New Size:", new_esc.as_str()),
            ("💾 Reclaimed:", rec_esc.as_str()),
            ("🖥 Host:", host_info.as_str()),
        ];
        format_card("DubStrip", "🗡️ <b>AUDIO DUB TRACKS STRIPPED</b>", &fields)
    };

    send_alert(&config.bot_token, &config.chat_id, &card)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_usage() {
        let one_gb = 1024 * 1024 * 1024;
        let s = format_usage(one_gb * 2, one_gb * 10);
        assert!(s.contains("2.0 / 10.0 GB (20.0%)"));
    }

    #[test]
    fn test_resolve_user_fallback() {
        let u = resolve_user(Some("alice"));
        assert_eq!(u, "alice");
    }

    #[test]
    fn test_audio_strip_card_formatting() {
        let fields = [
            ("🎬 Title:", "Test Movie (2026)"),
            ("🧠 Origin:", "Telugu"),
            ("🛡 Kept:", "Telugu 5.1"),
            ("🗑 Stripped:", "Tamil 5.1"),
            ("📦 Previous:", "4.50 GiB"),
            ("📦 New Size:", "3.10 GiB"),
            ("💾 Reclaimed:", "1.40 GiB"),
            ("🖥 Host:", "mochi"),
        ];
        let card = format_card("DubStrip", "🗡️ <b>AUDIO DUB TRACKS STRIPPED</b>", &fields);
        assert!(card.contains("DubStrip"));
        assert!(card.contains("Test Movie (2026)"));
        assert!(card.contains("1.40 GiB"));
    }
}
