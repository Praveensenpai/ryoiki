pub mod rtc;
pub mod systemd;

use anyhow::{bail, Result};
use colored::Colorize;

use crate::settings::{AutoPoweronSettings, MAX_AUTO_POWERON_DELAY, MIN_AUTO_POWERON_DELAY};

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
        "state: {}\nconfigured: {}\ndelay: {}s ({} min)\nshutdown hook: {}\nrtc: {}\narmed alarm: {}",
        if config.enabled { "on" } else { "off" },
        config.enabled,
        config.delay_secs,
        config.delay_secs / 60,
        unit,
        capable,
        alarm
    )
}

/// Returns the configured wake delay in seconds.
pub fn configured_delay() -> u32 {
    crate::settings::load_or_default().auto_power_on.delay_secs
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
}
