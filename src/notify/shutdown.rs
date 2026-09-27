use std::process::Command;
use std::time::Duration;

use anyhow::Result;

use super::client::{format_card, send_alert};
use super::config::TelegramConfig;

pub fn send_shutdown_notification(config: &TelegramConfig) -> Result<()> {
    let mut last_err = None;
    for attempt in 1..=3 {
        let card = build_shutdown_card(config);
        match send_alert(&config.bot_token, &config.chat_id, &card) {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                if attempt < 3 {
                    std::thread::sleep(Duration::from_secs(2));
                }
            }
        }
    }
    last_err.map_or(Ok(()), Err)
}

fn build_shutdown_card(config: &TelegramConfig) -> String {
    let host = get_hostname(config);
    let cause = detect_shutdown_cause();
    let uptime = super::system::get_uptime_str();
    let pct = super::power::read_battery_percent();
    let status = super::power::read_battery_status();
    let batt = match (pct, status) {
        (Some(p), Some(s)) => format!("{p}% ({s})"),
        (Some(p), None) => format!("{p}%"),
        _ => "N/A".to_string(),
    };
    let mem = super::system::get_memory_str();
    let disk = super::system::get_disk_str();

    let fields = [
        ("🖥 Host:", host.as_str()),
        ("🔍 Cause:", cause),
        ("⏱ Uptime:", uptime.as_str()),
        ("🔋 Battery:", batt.as_str()),
        ("🧠 Memory:", mem.as_str()),
        ("💾 Root Disk:", disk.as_str()),
    ];
    format_card("System Shutdown", "🔴 <b>SYSTEM GOING OFFLINE</b>", &fields)
}

/// Detects shutdown cause from sysfs + systemd.
/// Priority: battery-critical → reboot → power-off.
fn detect_shutdown_cause() -> &'static str {
    let is_battery_critical = super::power::read_battery_percent().is_some_and(|p| p <= 5)
        && super::power::read_battery_status()
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case("discharging"));
    if is_battery_critical {
        return "🪫 Battery Critical";
    }
    let is_reboot = Command::new("systemctl")
        .args(["is-active", "reboot.target"])
        .output()
        .is_ok_and(|o| o.stdout.starts_with(b"active"));
    if is_reboot {
        "🔄 Reboot"
    } else {
        "⏻ Power Off"
    }
}

fn get_hostname(config: &TelegramConfig) -> String {
    if let Some(name) = &config.server_name {
        return name.clone();
    }
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map_or_else(|_| "ryoiki".to_string(), |s| s.trim().to_string())
}
