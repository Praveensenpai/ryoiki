use anyhow::Result;
use std::fs;
use std::thread;
use std::time::Duration;

use super::client::{format_card, send_alert};
use super::config::TelegramConfig;

/// Battery thresholds that trigger a low-battery alert (descending order).
const LOW_BATTERY_THRESHOLDS: &[u8] = &[50, 40, 30, 25, 15, 5, 1];

/// How often the battery watcher polls (seconds).
const POLL_INTERVAL_SECS: u64 = 60;

// ── Public entry points ──────────────────────────────────────────────────────

/// Called by the udev rule when AC power is plugged or unplugged.
/// `status` should be `"plugged"` or `"unplugged"`.
pub fn send_power_event(config: &TelegramConfig, status: &str) -> Result<()> {
    let card = build_power_card(config, status);
    send_alert(&config.bot_token, &config.chat_id, &card)
}

/// Long-running daemon: polls battery every minute and fires a Telegram alert
/// the first time each low-battery threshold is crossed.
pub fn run_battery_watch(config: &TelegramConfig) -> Result<()> {
    let mut fired: Vec<u8> = Vec::new();

    loop {
        if let Some(pct) = read_battery_percent() {
            for &threshold in LOW_BATTERY_THRESHOLDS {
                if pct <= threshold && !fired.contains(&threshold) {
                    fired.push(threshold);
                    let card = build_battery_alert_card(config, pct, threshold);
                    let _ = send_alert(&config.bot_token, &config.chat_id, &card);
                }
            }
        }
        thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));
    }
}

// ── Card builders ────────────────────────────────────────────────────────────

fn build_power_card(config: &TelegramConfig, status: &str) -> String {
    let (badge, icon) = if status == "plugged" {
        ("🔌 <b>AC POWER CONNECTED</b>", "🔌")
    } else {
        ("🔋 <b>AC POWER DISCONNECTED</b>", "🔋")
    };

    let host = get_hostname(config);
    let batt = read_battery_percent()
        .map_or_else(|| "N/A".to_string(), |p| format!("{p}%"));
    let batt_status = read_battery_status().unwrap_or_else(|| "unknown".to_string());
    let cpu_temp = read_cpu_temp().map_or_else(|| "N/A".to_string(), |t| format!("{t:.1} °C"));
    let cpu_usage = read_cpu_usage().map_or_else(|| "N/A".to_string(), |u| format!("{u:.1}%"));

    let event_label = format!("{icon} Power {status}");

    let fields = [
        ("🖥 Host:", host.as_str()),
        ("⚡ Event:", event_label.as_str()),
        ("🔋 Battery:", batt.as_str()),
        ("📊 Batt Status:", batt_status.as_str()),
        ("🌡 CPU Temp:", cpu_temp.as_str()),
        ("⚙ CPU Usage:", cpu_usage.as_str()),
    ];

    format_card("Power Event", badge, &fields)
}

fn build_battery_alert_card(config: &TelegramConfig, pct: u8, threshold: u8) -> String {
    let badge = if threshold <= 5 {
        "🚨 <b>CRITICAL BATTERY</b>"
    } else if threshold <= 15 {
        "⚠️ <b>LOW BATTERY</b>"
    } else {
        "🔋 <b>BATTERY ALERT</b>"
    };

    let host = get_hostname(config);
    let pct_str = format!("{pct}%");
    let threshold_str = format!("{threshold}%");
    let cpu_temp = read_cpu_temp().map_or_else(|| "N/A".to_string(), |t| format!("{t:.1} °C"));
    let cpu_usage = read_cpu_usage().map_or_else(|| "N/A".to_string(), |u| format!("{u:.1}%"));
    let batt_status = read_battery_status().unwrap_or_else(|| "unknown".to_string());

    let fields = [
        ("🖥 Host:", host.as_str()),
        ("🔋 Battery:", pct_str.as_str()),
        ("🎯 Threshold:", threshold_str.as_str()),
        ("📊 Batt Status:", batt_status.as_str()),
        ("🌡 CPU Temp:", cpu_temp.as_str()),
        ("⚙ CPU Usage:", cpu_usage.as_str()),
    ];

    format_card("Battery Warning", badge, &fields)
}

// ── System readers ────────────────────────────────────────────────────────────

/// Read battery percentage from the first battery found in `/sys/class/power_supply`/.
fn read_battery_percent() -> Option<u8> {
    let base = "/sys/class/power_supply";
    for entry in fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        let type_path = path.join("type");
        let kind = fs::read_to_string(&type_path)
            .map(|s| s.trim().to_lowercase())
            .unwrap_or_default();
        if kind != "battery" {
            continue;
        }
        let cap = fs::read_to_string(path.join("capacity"))
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok());
        return cap;
    }
    None
}

/// Read battery charge status ("Charging", "Discharging", "Full", …).
fn read_battery_status() -> Option<String> {
    let base = "/sys/class/power_supply";
    for entry in fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        let type_path = path.join("type");
        let kind = fs::read_to_string(&type_path)
            .map(|s| s.trim().to_lowercase())
            .unwrap_or_default();
        if kind != "battery" {
            continue;
        }
        let status = fs::read_to_string(path.join("status"))
            .map(|s| s.trim().to_string())
            .ok();
        return status;
    }
    None
}

/// Read CPU temperature from hwmon or `thermal_zone` sysfs.
fn read_cpu_temp() -> Option<f64> {
    // Try hwmon first (more accurate on most systems)
    if let Some(t) = read_hwmon_temp() {
        return Some(t);
    }
    // Fall back to thermal_zone
    for i in 0..10 {
        let path = format!("/sys/class/thermal/thermal_zone{i}/temp");
        if let Ok(s) = fs::read_to_string(&path) {
            if let Ok(millic) = s.trim().parse::<u32>() {
                return Some(f64::from(millic) / 1000.0);
            }
        }
    }
    None
}

fn read_hwmon_temp() -> Option<f64> {
    let base = "/sys/class/hwmon";
    for entry in fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        // Look for temp1_input (primary CPU sensor)
        let temp_path = path.join("temp1_input");
        if let Ok(s) = fs::read_to_string(&temp_path) {
            if let Ok(millic) = s.trim().parse::<u32>() {
                let t = f64::from(millic) / 1000.0;
                // Sanity check: plausible CPU temp
                if (0.0..150.0).contains(&t) {
                    return Some(t);
                }
            }
        }
    }
    None
}

/// Read instantaneous CPU usage by sampling /proc/stat twice with a 200 ms gap.
fn read_cpu_usage() -> Option<f64> {
    let (idle1, total1) = parse_cpu_stat()?;
    thread::sleep(Duration::from_millis(200));
    let (idle2, total2) = parse_cpu_stat()?;

    let delta_total = total2.saturating_sub(total1);
    let delta_idle = idle2.saturating_sub(idle1);

    if delta_total == 0 {
        return Some(0.0);
    }
    #[allow(clippy::cast_precision_loss)]
    let usage = 100.0 * (1.0 - delta_idle as f64 / delta_total as f64);
    Some(usage.clamp(0.0, 100.0))
}

fn parse_cpu_stat() -> Option<(u64, u64)> {
    let content = fs::read_to_string("/proc/stat").ok()?;
    let line = content.lines().next()?; // first line: "cpu  ..."
    let mut parts = line.split_whitespace();
    parts.next(); // skip "cpu" label
    let values: Vec<u64> = parts.filter_map(|v| v.parse().ok()).collect();
    if values.len() < 4 {
        return None;
    }
    let idle = values[3]; // idle jiffies
    let total: u64 = values.iter().sum();
    Some((idle, total))
}

fn get_hostname(config: &super::config::TelegramConfig) -> String {
    if let Some(name) = &config.server_name {
        return name.clone();
    }
    fs::read_to_string("/proc/sys/kernel/hostname")
        .map_or_else(|_| "ryoiki".to_string(), |s| s.trim().to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_low_battery_thresholds_descending() {
        let mut prev = 255u8;
        for &t in LOW_BATTERY_THRESHOLDS {
            assert!(t < prev, "Thresholds must be in descending order");
            prev = t;
        }
    }

    #[test]
    fn test_parse_cpu_stat_reads_proc() {
        // /proc/stat is always present on Linux
        let result = parse_cpu_stat();
        assert!(result.is_some(), "parse_cpu_stat should succeed on Linux");
        if let Some((idle, total)) = result {
            assert!(total >= idle, "total must be >= idle jiffies");
        }
    }

    #[test]
    fn test_cpu_usage_in_range() {
        if let Some(usage) = read_cpu_usage() {
            assert!((0.0..=100.0).contains(&usage));
        }
    }
}
