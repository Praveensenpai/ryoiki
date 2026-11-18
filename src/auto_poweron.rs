pub mod rtc;
pub mod systemd;

use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

use crate::settings::{AutoPoweronSettings, MAX_AUTO_POWERON_DELAY, MIN_AUTO_POWERON_DELAY};

/// Runtime-only override written before a low-battery shutdown (tmpfs, cleared on boot).
pub const MARKER_PATH: &str = "/run/ryoiki/poweroff-delay.conf";

/// Provisions auto power-on from persisted settings during a full install run.
pub fn run() -> Result<()> {
    let config = crate::settings::load_or_default().auto_power_on;
    systemd::setup(config.enabled, config.delay_secs)?;
    println!(
        "  {} RTC auto power-on enabled={} delay={}s (persists across reboots)",
        "✔".green().bold(),
        config.enabled,
        config.delay_secs
    );
    Ok(())
}

/// Applies a full auto power-on configuration and persists it to settings.
pub fn apply_programmatic(enable: bool, delay_secs: u32) -> Result<String> {
    validate_delay(delay_secs)?;
    if enable {
        rtc::set_alarm(delay_secs)?;
    }
    systemd::setup(enable, delay_secs)?;

    let mut settings = crate::settings::load_or_default();
    settings.auto_power_on = AutoPoweronSettings {
        enabled: enable,
        delay_secs,
        battery_delay_secs: settings.auto_power_on.battery_delay_secs,
    };
    crate::settings::save(&settings)?;

    let state = if enable { "enabled" } else { "disabled" };
    Ok(format!(
        "RTC auto power-on {state} with a {delay_secs}s wake delay."
    ))
}

/// Returns a human-readable status snapshot of the feature.
pub fn status() -> String {
    let config = crate::settings::load_or_default().auto_power_on;
    let unit = if systemd::unit_enabled() {
        "enabled"
    } else {
        "disabled"
    };
    let capable = if rtc::supported() {
        "supported"
    } else {
        "unavailable (rtcwake or RTC sysfs missing)"
    };
    let alarm =
        rtc::read_wakealarm().map_or_else(|| "none armed".to_string(), |ts| format!("{ts} (unix)"));
    format!(
        "state: {}\nconfigured: {}\ndelay: {}s ({} min)\nbattery delay: {}s ({} min)\nshutdown hook: {}\nrtc: {}\narmed alarm: {}",
        if config.enabled { "on" } else { "off" },
        config.enabled,
        config.delay_secs,
        config.delay_secs / 60,
        config.battery_delay_secs,
        config.battery_delay_secs / 60,
        unit,
        capable,
        alarm
    )
}

/// Updates the low-battery wake delay and persists it.
pub fn set_battery_delay(delay_secs: u32) -> Result<String> {
    validate_delay(delay_secs)?;
    let mut settings = crate::settings::load_or_default();
    settings.auto_power_on.battery_delay_secs = delay_secs;
    crate::settings::save(&settings)?;
    Ok(format!("Low-battery wake delay set to {delay_secs}s."))
}

/// Returns the configured wake delay in seconds.
pub fn configured_delay() -> u32 {
    crate::settings::load_or_default().auto_power_on.delay_secs
}

/// Returns the configured low-battery wake delay in seconds.
pub fn configured_battery_delay() -> u32 {
    crate::settings::load_or_default()
        .auto_power_on
        .battery_delay_secs
}

/// Builds the content written to the runtime delay marker.
fn marker_contents(delay_secs: u32) -> String {
    format!("RYOIKI_DELAY={delay_secs}\n")
}

/// Arms the longer retry delay for a low-battery shutdown.
pub fn write_battery_marker() -> Result<()> {
    let delay = configured_battery_delay();
    validate_delay(delay)?;
    let path = Path::new(MARKER_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Could not create /run/ryoiki")?;
    }
    fs::write(path, marker_contents(delay)).context("Could not write poweroff delay marker")?;
    Ok(())
}

/// Handles the CLI action and optional value args from the parser.
pub fn handle_cli_args(action: &str, value: Option<&str>) -> Result<()> {
    let combined = value.map_or_else(|| action.to_string(), |v| format!("{action} {v}"));
    handle_cli(&combined)
}

/// Handles the CLI action string: status, on, off, or a numeric delay.
pub fn handle_cli(action: &str) -> Result<()> {
    let action = action.trim();
    if action.is_empty() || action.eq_ignore_ascii_case("status") {
        println!("{}", status());
        return Ok(());
    }
    if action.eq_ignore_ascii_case("on") {
        println!("{}", apply_programmatic(true, configured_delay())?);
        return Ok(());
    }
    if action.eq_ignore_ascii_case("off") {
        println!("{}", apply_programmatic(false, configured_delay())?);
        return Ok(());
    }
    if let Some(value) = action.strip_prefix("battery").map(str::trim) {
        if value.is_empty() {
            println!("{}", configured_battery_delay());
            return Ok(());
        }
        let secs = value
            .parse::<u32>()
            .map_err(|_| anyhow::anyhow!("Use 'battery <seconds>'"))?;
        println!("{}", set_battery_delay(secs)?);
        return Ok(());
    }
    let secs = action
        .parse::<u32>()
        .map_err(|_| anyhow::anyhow!("Use 'status', 'on', 'off', or a delay in seconds"))?;
    println!("{}", apply_programmatic(true, secs)?);
    Ok(())
}

fn validate_delay(secs: u32) -> Result<()> {
    if !(MIN_AUTO_POWERON_DELAY..=MAX_AUTO_POWERON_DELAY).contains(&secs) {
        bail!(
            "delay must be {MIN_AUTO_POWERON_DELAY}-{MAX_AUTO_POWERON_DELAY} seconds, got {secs}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_delay_bounds() {
        assert!(validate_delay(MIN_AUTO_POWERON_DELAY).is_ok());
        assert!(validate_delay(300).is_ok());
        assert!(validate_delay(MAX_AUTO_POWERON_DELAY).is_ok());
        assert!(validate_delay(0).is_err());
        assert!(validate_delay(9).is_err());
        assert!(validate_delay(86_401).is_err());
    }

    #[test]
    fn test_marker_contents() {
        assert_eq!(marker_contents(900), "RYOIKI_DELAY=900\n");
        assert_eq!(marker_contents(300), "RYOIKI_DELAY=300\n");
    }

    #[test]
    fn test_write_battery_marker_writes_runtime_override() {
        if write_battery_marker().is_err() {
            return;
        }
        let Ok(content) = fs::read_to_string(MARKER_PATH) else {
            return;
        };
        assert_eq!(content, marker_contents(configured_battery_delay()));
        let _ = fs::remove_file(MARKER_PATH);
    }
}
