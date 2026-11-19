use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::thread;
use std::time::{Duration, SystemTime};

use super::client::{format_card, send_alert};
use super::config::TelegramConfig;
use crate::settings::Settings;

mod lock;
mod sensors;
use lock::acquire_pid_lock;
pub use sensors::{
    is_ac_online, read_battery_percent, read_battery_status, read_cpu_temp, read_cpu_usage,
    sample_cpu_usage,
};

/// How often the battery watcher polls (seconds).
const POLL_INTERVAL_SECS: u64 = 60;

/// File under `~/.local/state/ryoiki` that records shutdown decisions.
const POWEROFF_LOG: &str = "power.log";

/// Environment escape hatch used by smoke tests to suppress a real shutdown.
const NO_POWEROFF_ENV: &str = "RYOIKI_NO_POWEROFF";

// ── Public entry points ──────────────────────────────────────────────────────

/// Called by the udev rule when AC power is plugged or unplugged.
/// `status` should be `"plugged"` or `"unplugged"`.
pub fn send_power_event(config: &TelegramConfig, status: &str) -> Result<()> {
    let card = build_power_card(config, status);
    send_alert(&config.bot_token, &config.chat_id, &card)
}

/// Long-running daemon: polls battery every minute.
/// - When on HP ACPI hardware, actively regulates charging to keep battery at target limit.
/// - When charging on AC without ACPI hooks, fires an alert when target limit is reached.
/// - When on battery, fires a Telegram alert at each configured low-battery threshold.
/// - When enabled, powers the host off once the configured critical threshold is reached.
///
/// Acquires an exclusive PID lock file on startup. A second instance will detect the lock
/// and exit cleanly, preventing duplicate Telegram notifications.
pub fn run_battery_watch(config: &TelegramConfig) -> Result<()> {
    let _pid_guard = acquire_pid_lock()?;
    let mut state = WatchState::default();

    loop {
        let settings = crate::settings::load_or_default();
        if let Some(pct) = read_battery_percent() {
            update_charge_state(config, &settings, pct, is_ac_online(), &mut state);
        }
        thread::sleep(Duration::from_secs(POLL_INTERVAL_SECS));
    }
}

#[derive(Default)]
struct WatchState {
    fired_low: Vec<u8>,
    fired_high: bool,
    poweroff_fired: bool,
    last_hp_mode: Option<crate::charge_limit::hp_acpi::HpChargeMode>,
}

fn update_charge_state(
    config: &TelegramConfig,
    settings: &Settings,
    pct: u8,
    ac_online: bool,
    state: &mut WatchState,
) {
    let target_limit = settings.battery.max_charge_percent;

    let ceiling_enabled = settings.battery.max_charge_enabled;
    if ceiling_enabled {
        if crate::charge_limit::hp_acpi::is_hp_acpi_supported() {
            regulate_hp(config, pct, ac_online, target_limit, state);
        } else if ac_online && pct >= target_limit && !state.fired_high {
            state.fired_high = true;
            let card = build_high_charge_card(config, pct, target_limit);
            let _ = send_alert(&config.bot_token, &config.chat_id, &card);
        }
    } else {
        state.fired_high = false;
    }

    if ac_online {
        state.fired_low.retain(|&threshold| pct <= threshold);
        state.poweroff_fired = false;
        return;
    }

    state.fired_high = false;
    fire_low_alerts(
        config,
        pct,
        &settings.battery.alert_thresholds,
        &mut state.fired_low,
    );
    maybe_power_off(config, pct, settings, state);
}

fn regulate_hp(
    config: &TelegramConfig,
    pct: u8,
    ac_online: bool,
    target_limit: u8,
    state: &mut WatchState,
) {
    let Ok(new_mode) =
        crate::charge_limit::hp_acpi::regulate_hp_battery(target_limit, pct, ac_online)
    else {
        return;
    };
    if state.last_hp_mode == Some(new_mode) {
        return;
    }
    state.last_hp_mode = Some(new_mode);
    let card = build_hp_acpi_card(config, pct, target_limit, new_mode);
    let _ = send_alert(&config.bot_token, &config.chat_id, &card);
}

fn maybe_power_off(config: &TelegramConfig, pct: u8, settings: &Settings, state: &mut WatchState) {
    if !should_power_off(pct, false, settings, state.poweroff_fired) {
        return;
    }
    state.poweroff_fired = true;
    let threshold = settings.power.poweroff_percent;
    let card = build_poweroff_card(config, pct, threshold);
    let _ = send_alert(&config.bot_token, &config.chat_id, &card);
    execute_poweroff(pct, threshold);
}

/// Emits one alert per configured threshold while the battery is discharging.
fn fire_low_alerts(config: &TelegramConfig, pct: u8, thresholds: &[u8], fired_low: &mut Vec<u8>) {
    for &threshold in thresholds {
        if pct <= threshold && !fired_low.contains(&threshold) {
            fired_low.push(threshold);
            let card = build_battery_alert_card(config, pct, threshold);
            let _ = send_alert(&config.bot_token, &config.chat_id, &card);
        }
    }
}

/// Pure decision predicate so the shutdown behaviour is unit-testable.
#[must_use]
pub fn should_power_off(
    pct: u8,
    ac_online: bool,
    settings: &Settings,
    already_fired: bool,
) -> bool {
    settings.power.poweroff_on_low
        && !already_fired
        && !ac_online
        && pct <= settings.power.poweroff_percent
}

fn execute_poweroff(pct: u8, threshold: u8) {
    log_poweroff(pct, threshold);
    if std::env::var_os(NO_POWEROFF_ENV).is_some() {
        return;
    }
    // Arm the longer retry interval so the host comes back every 15 min
    // until AC power is restored. /run is tmpfs, so this clears on boot.
    let _ = crate::auto_poweron::write_battery_marker();
    let direct = std::process::Command::new("systemctl")
        .arg("poweroff")
        .output();
    if direct.is_ok_and(|out| out.status.success()) {
        return;
    }
    let _ = std::process::Command::new("sudo")
        .args(["-n", "systemctl", "poweroff"])
        .output();
}

fn log_poweroff(pct: u8, threshold: u8) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let dir = Path::new(&home).join(".local/state/ryoiki");
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let line = format!(
        "{:?} battery={pct}% threshold={threshold}% action=poweroff\n",
        SystemTime::now()
    );
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(POWEROFF_LOG))
    {
        let _ = file.write_all(line.as_bytes());
    }
}

// PID locking lives in `lock.rs` so the stale-lock logic stays unit-testable.

// ── Card builders ────────────────────────────────────────────────────────────

fn build_power_card(config: &TelegramConfig, status: &str) -> String {
    let (badge, icon) = if status == "plugged" {
        ("🔌 <b>AC POWER CONNECTED</b>", "🔌")
    } else {
        ("🔋 <b>AC POWER DISCONNECTED</b>", "🔋")
    };

    let host = get_hostname(config);
    let batt = read_battery_percent().map_or_else(|| "N/A".to_string(), |p| format!("{p}%"));
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

fn build_high_charge_card(config: &TelegramConfig, pct: u8, limit: u8) -> String {
    let badge = "🔋 <b>CHARGE LIMIT REACHED</b>";
    let host = get_hostname(config);
    let pct_str = format!("{pct}%");
    let limit_str = format!("{limit}%");
    let cpu_temp = read_cpu_temp().map_or_else(|| "N/A".to_string(), |t| format!("{t:.1} °C"));
    let cpu_usage = read_cpu_usage().map_or_else(|| "N/A".to_string(), |u| format!("{u:.1}%"));
    let batt_status = read_battery_status().unwrap_or_else(|| "unknown".to_string());

    let fields = [
        ("🖥 Host:", host.as_str()),
        ("⚡ Action:", "Unplug AC charger to preserve lifespan"),
        ("🔋 Battery:", pct_str.as_str()),
        ("🎯 Target Limit:", limit_str.as_str()),
        ("📊 Batt Status:", batt_status.as_str()),
        ("🌡 CPU Temp:", cpu_temp.as_str()),
        ("⚙ CPU Usage:", cpu_usage.as_str()),
    ];

    format_card("Charge Limit Alert", badge, &fields)
}

fn build_poweroff_card(config: &TelegramConfig, pct: u8, threshold: u8) -> String {
    let host = get_hostname(config);
    let pct_str = format!("{pct}%");
    let threshold_str = format!("{threshold}%");
    let batt_status = read_battery_status().unwrap_or_else(|| "unknown".to_string());

    let fields = [
        ("🖥 Host:", host.as_str()),
        ("⚡ Action:", "Powering off to protect the battery"),
        ("🔋 Battery:", pct_str.as_str()),
        ("🎯 Shutdown Threshold:", threshold_str.as_str()),
        ("📊 Batt Status:", batt_status.as_str()),
    ];

    format_card("Critical Battery", "🛑 <b>EMERGENCY SHUTDOWN</b>", &fields)
}

fn build_hp_acpi_card(
    config: &TelegramConfig,
    pct: u8,
    limit: u8,
    mode: crate::charge_limit::hp_acpi::HpChargeMode,
) -> String {
    let host = get_hostname(config);
    let pct_str = format!("{pct}%");
    let limit_str = format!("{limit}%");
    let cpu_temp = read_cpu_temp().map_or_else(|| "N/A".to_string(), |t| format!("{t:.1} °C"));
    let cpu_usage = read_cpu_usage().map_or_else(|| "N/A".to_string(), |u| format!("{u:.1}%"));
    let batt_status = read_battery_status().unwrap_or_else(|| "unknown".to_string());

    let fields = [
        ("🖥 Host:", host.as_str()),
        ("⚙ State:", mode.label()),
        ("🔋 Battery:", pct_str.as_str()),
        ("🎯 Target Limit:", limit_str.as_str()),
        ("📊 Batt Status:", batt_status.as_str()),
        ("🌡 CPU Temp:", cpu_temp.as_str()),
        ("⚙ CPU Usage:", cpu_usage.as_str()),
    ];

    format_card(
        "HP Charge Regulation",
        "⚡ <b>FIRMWARE CHARGE CONTROL</b>",
        &fields,
    )
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
        let settings = Settings::default();
        let mut prev = 255u8;
        for &t in &settings.battery.alert_thresholds {
            assert!(t < prev, "Thresholds must be in descending order");
            prev = t;
        }
    }

    #[test]
    fn test_should_power_off_disabled_by_default() {
        let settings = Settings::default();
        assert!(!should_power_off(1, false, &settings, false));
    }

    #[test]
    fn test_should_power_off_only_when_on_battery_and_crossed() {
        let mut settings = Settings::default();
        settings.power.poweroff_on_low = true;
        settings.power.poweroff_percent = 10;
        assert!(should_power_off(10, false, &settings, false));
        assert!(should_power_off(3, false, &settings, false));
        assert!(!should_power_off(11, false, &settings, false));
        assert!(!should_power_off(5, true, &settings, false));
        assert!(!should_power_off(5, false, &settings, true));
    }
}
