use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// Smallest accepted max-charge value.
pub const MIN_CHARGE: u8 = 20;
/// Largest accepted max-charge value.
pub const MAX_CHARGE: u8 = 100;
/// Smallest accepted auto-poweroff value.
pub const MIN_POWEROFF: u8 = 1;
/// Largest accepted auto-poweroff value.
pub const MAX_POWEROFF: u8 = 50;
/// Smallest accepted media-prune trigger.
pub const MIN_PRUNE: u8 = 60;
/// Largest accepted media-prune trigger.
pub const MAX_PRUNE: u8 = 95;
/// Smallest accepted media-prune target.
pub const MIN_PRUNE_TARGET: u8 = 50;
/// Minimum distance between the prune trigger and its target.
pub const MIN_PRUNE_GAP: u8 = 5;
/// Default media-prune trigger.
pub const DEFAULT_PRUNE_THRESHOLD: u8 = 90;
/// Default media-prune recovery target.
pub const DEFAULT_PRUNE_TARGET: u8 = 75;
/// Default low-battery alert ladder, descending.
pub const DEFAULT_ALERTS: [u8; 7] = [50, 40, 30, 25, 15, 5, 1];
/// Default UI refresh interval in seconds.
pub const DEFAULT_REFRESH_SECS: u8 = 5;
/// Smallest accepted RTC auto power-on wake delay.
pub const MIN_AUTO_POWERON_DELAY: u32 = 10;
/// Largest accepted RTC auto power-on wake delay (24 hours).
pub const MAX_AUTO_POWERON_DELAY: u32 = 86_400;
/// Default RTC auto power-on wake delay (5 minutes).
pub const DEFAULT_AUTO_POWERON_DELAY: u32 = 300;
/// Default wake delay after a low-battery shutdown (15 minutes).
pub const DEFAULT_BATTERY_DELAY: u32 = 900;

const fn default_true() -> bool {
    true
}

const fn default_charge() -> u8 {
    60
}

const fn default_poweroff() -> u8 {
    5
}

const fn default_refresh() -> u8 {
    DEFAULT_REFRESH_SECS
}

fn default_alerts() -> Vec<u8> {
    DEFAULT_ALERTS.to_vec()
}

const fn default_prune_threshold() -> u8 {
    DEFAULT_PRUNE_THRESHOLD
}

const fn default_prune_target() -> u8 {
    DEFAULT_PRUNE_TARGET
}

const fn default_auto_poweron_delay() -> u32 {
    DEFAULT_AUTO_POWERON_DELAY
}

const fn default_battery_delay() -> u32 {
    DEFAULT_BATTERY_DELAY
}

/// Battery charging and alert behaviour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatterySettings {
    #[serde(default = "default_true")]
    pub max_charge_enabled: bool,
    #[serde(default = "default_charge")]
    pub max_charge_percent: u8,
    #[serde(default = "default_alerts")]
    pub alert_thresholds: Vec<u8>,
}

impl Default for BatterySettings {
    fn default() -> Self {
        Self {
            max_charge_enabled: true,
            max_charge_percent: default_charge(),
            alert_thresholds: default_alerts(),
        }
    }
}

/// Automatic low-battery shutdown behaviour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerSettings {
    #[serde(default)]
    pub poweroff_on_low: bool,
    #[serde(default = "default_poweroff")]
    pub poweroff_percent: u8,
}

impl Default for PowerSettings {
    fn default() -> Self {
        Self {
            poweroff_on_low: false,
            poweroff_percent: default_poweroff(),
        }
    }
}

/// Terminal interface preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSettings {
    #[serde(default = "default_refresh")]
    pub refresh_secs: u8,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            refresh_secs: default_refresh(),
        }
    }
}

/// Media storage auto-prune policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageSettings {
    #[serde(default = "default_prune_threshold")]
    pub prune_threshold: u8,
    #[serde(default = "default_prune_target")]
    pub prune_target: u8,
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            prune_threshold: default_prune_threshold(),
            prune_target: default_prune_target(),
        }
    }
}

/// RTC wake behaviour applied after every system poweroff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoPoweronSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_auto_poweron_delay")]
    pub delay_secs: u32,
    #[serde(default = "default_battery_delay")]
    pub battery_delay_secs: u32,
}

impl Default for AutoPoweronSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            delay_secs: default_auto_poweron_delay(),
            battery_delay_secs: default_battery_delay(),
        }
    }
}

/// Root persisted configuration for ryoiki.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub battery: BatterySettings,
    #[serde(default)]
    pub power: PowerSettings,
    #[serde(default)]
    pub storage: StorageSettings,
    #[serde(default)]
    pub ui: UiSettings,
    #[serde(default)]
    pub auto_power_on: AutoPoweronSettings,
}

impl Settings {
    /// Rejects out-of-range values before they are persisted.
    pub fn validate(&self) -> Result<()> {
        if !(MIN_CHARGE..=MAX_CHARGE).contains(&self.battery.max_charge_percent) {
            bail!(
                "max charge must be {MIN_CHARGE}-{MAX_CHARGE}%, got {}%",
                self.battery.max_charge_percent
            );
        }
        validate_poweroff(self.power.poweroff_percent)?;
        validate_prune(self.storage.prune_threshold, self.storage.prune_target)?;
        validate_auto_poweron_delay(self.auto_power_on.delay_secs)?;
        validate_auto_poweron_delay(self.auto_power_on.battery_delay_secs)?;
        validate_thresholds(&self.battery.alert_thresholds)?;
        if !(1..=120).contains(&self.ui.refresh_secs) {
            bail!(
                "refresh interval must be 1-120s, got {}s",
                self.ui.refresh_secs
            );
        }
        Ok(())
    }

    /// Parses a comma or space separated threshold list into a descending vector.
    pub fn parse_thresholds(raw: &str) -> Result<Vec<u8>> {
        let mut values = Vec::new();
        for token in raw.split([',', ' ', ';']).filter(|t| !t.is_empty()) {
            let value = token
                .trim()
                .parse::<u8>()
                .map_err(|_| anyhow::anyhow!("'{token}' is not a percentage"))?;
            if !(1..=100).contains(&value) {
                bail!("threshold {value}% must be 1-100%");
            }
            values.push(value);
        }
        if values.is_empty() {
            bail!("provide at least one threshold");
        }
        values.sort_unstable_by(|a, b| b.cmp(a));
        values.dedup();
        Ok(values)
    }
}

fn validate_poweroff(value: u8) -> Result<()> {
    if !(MIN_POWEROFF..=MAX_POWEROFF).contains(&value) {
        bail!("poweroff threshold must be {MIN_POWEROFF}-{MAX_POWEROFF}%, got {value}%");
    }
    Ok(())
}

fn validate_auto_poweron_delay(secs: u32) -> Result<()> {
    if !(MIN_AUTO_POWERON_DELAY..=MAX_AUTO_POWERON_DELAY).contains(&secs) {
        bail!(
            "auto power-on delay must be {MIN_AUTO_POWERON_DELAY}-{MAX_AUTO_POWERON_DELAY}s, got {secs}"
        );
    }
    Ok(())
}

/// Validates a media-prune trigger and recovery target.
pub fn validate_prune(threshold: u8, target: u8) -> Result<()> {
    if !(MIN_PRUNE..=MAX_PRUNE).contains(&threshold) {
        bail!("prune threshold must be {MIN_PRUNE}-{MAX_PRUNE}%, got {threshold}%");
    }
    if !(MIN_PRUNE_TARGET..=100).contains(&target) {
        bail!("prune target must be {MIN_PRUNE_TARGET}-100%, got {target}%");
    }
    if target > threshold.saturating_sub(MIN_PRUNE_GAP) {
        bail!(
            "prune target must be at least {MIN_PRUNE_GAP} points below the {threshold}% threshold, got {target}%"
        );
    }
    Ok(())
}

fn validate_thresholds(values: &[u8]) -> Result<()> {
    if values.is_empty() {
        bail!("at least one battery alert threshold is required");
    }
    for &value in values {
        if !(1..=100).contains(&value) {
            bail!("alert threshold {value}% must be 1-100%");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_are_valid() {
        assert!(Settings::default().validate().is_ok());
    }

    #[test]
    fn test_default_prune_policy() {
        let storage = Settings::default().storage;
        assert_eq!(storage.prune_threshold, 90);
        assert_eq!(storage.prune_target, 75);
    }

    #[test]
    fn test_rejects_bad_prune_threshold() {
        let mut settings = Settings::default();
        settings.storage.prune_threshold = 59;
        assert!(settings.validate().is_err());

        settings.storage.prune_threshold = 96;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_rejects_prune_target_too_close() {
        assert!(validate_prune(90, 86).is_err());
        assert!(validate_prune(90, 85).is_ok());
        assert!(validate_prune(90, 91).is_err());
    }

    #[test]
    fn test_rejects_prune_target_below_floor() {
        assert!(validate_prune(60, 49).is_err());
        assert!(validate_prune(60, 55).is_ok());
    }

    #[test]
    fn test_serde_without_storage_uses_defaults() {
        let settings: Settings = serde_json::from_str("{}").unwrap_or_default();
        assert_eq!(settings.storage, StorageSettings::default());
    }

    #[test]
    fn test_rejects_bad_charge() {
        let mut settings = Settings::default();
        settings.battery.max_charge_percent = 10;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_rejects_bad_poweroff() {
        let mut settings = Settings::default();
        settings.power.poweroff_percent = 80;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_auto_poweron_defaults_are_valid() {
        let auto = Settings::default().auto_power_on;
        assert!(auto.enabled);
        assert_eq!(auto.delay_secs, 300);
        assert_eq!(auto.battery_delay_secs, 900);
        assert!(Settings::default().validate().is_ok());
    }

    #[test]
    fn test_rejects_bad_battery_delay() {
        let mut settings = Settings::default();
        settings.auto_power_on.battery_delay_secs = 9;
        assert!(settings.validate().is_err());
        settings.auto_power_on.battery_delay_secs = 86_401;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_rejects_bad_auto_poweron_delay() {
        let mut settings = Settings::default();
        settings.auto_power_on.delay_secs = 5;
        assert!(settings.validate().is_err());
        settings.auto_power_on.delay_secs = 86_401;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_auto_poweron_defaults_when_missing() {
        let settings: Settings = serde_json::from_str("{}").unwrap_or_default();
        assert_eq!(settings.auto_power_on, Settings::default().auto_power_on);
    }

    #[test]
    fn test_parse_thresholds_sorts_descending() {
        let parsed = Settings::parse_thresholds("5, 50, 25 25").unwrap_or_default();
        assert_eq!(parsed, vec![50, 25, 5]);
    }

    #[test]
    fn test_parse_thresholds_rejects_empty() {
        assert!(Settings::parse_thresholds("  ").is_err());
    }

    #[test]
    fn test_serde_round_trip() {
        let mut settings = Settings::default();
        settings.power.poweroff_on_low = true;
        settings.battery.alert_thresholds = vec![40, 20];
        settings.auto_power_on.enabled = false;
        settings.auto_power_on.delay_secs = 600;
        settings.storage.prune_threshold = 70;
        settings.storage.prune_target = 60;
        let json = serde_json::to_string(&settings).unwrap_or_default();
        let loaded: Settings = serde_json::from_str(&json).unwrap_or_default();
        assert_eq!(loaded, settings);
    }
}
