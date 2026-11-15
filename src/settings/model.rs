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
/// Default low-battery alert ladder, descending.
pub const DEFAULT_ALERTS: [u8; 7] = [50, 40, 30, 25, 15, 5, 1];
/// Default UI refresh interval in seconds.
pub const DEFAULT_REFRESH_SECS: u8 = 5;

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

/// Root persisted configuration for ryoiki.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub battery: BatterySettings,
    #[serde(default)]
    pub power: PowerSettings,
    #[serde(default)]
    pub ui: UiSettings,
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
        let json = serde_json::to_string(&settings).unwrap_or_default();
        let loaded: Settings = serde_json::from_str(&json).unwrap_or_default();
        assert_eq!(loaded, settings);
    }
}
