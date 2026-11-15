use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::model::Settings;

/// Returns `~/.config/ryoiki`.
#[must_use]
pub fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    Path::new(&home).join(".config/ryoiki")
}

/// Returns the canonical settings path.
#[must_use]
pub fn path() -> PathBuf {
    config_dir().join("settings.json")
}

fn legacy_charge_path() -> Option<PathBuf> {
    let user_path = config_dir().join("charge_limit");
    if user_path.exists() {
        return Some(user_path);
    }
    let system_path = PathBuf::from("/etc/ryoiki/charge_limit");
    system_path.exists().then_some(system_path)
}

/// Loads settings from disk, parsing errors are returned to the caller.
pub fn load() -> Result<Settings> {
    let path = path();
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let settings: Settings = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    settings.validate()?;
    Ok(settings)
}

/// Loads settings, falling back to defaults and migrating the legacy charge file.
#[must_use]
pub fn load_or_default() -> Settings {
    if let Ok(settings) = load() {
        return settings;
    }
    let mut settings = Settings::default();
    if let Some(value) = legacy_charge_percent() {
        settings.battery.max_charge_percent = value;
    }
    settings
}

fn legacy_charge_percent() -> Option<u8> {
    let raw = fs::read_to_string(legacy_charge_path()?).ok()?;
    let value = raw.trim().parse::<u8>().ok()?;
    (super::MIN_CHARGE..=super::MAX_CHARGE)
        .contains(&value)
        .then_some(value)
}

/// Atomically writes settings after validating them.
pub fn save(settings: &Settings) -> Result<PathBuf> {
    settings.validate()?;
    let dir = config_dir();
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    let target = dir.join("settings.json");
    let staging = dir.join("settings.json.tmp");
    let json = serde_json::to_string_pretty(settings).context("failed to encode settings")?;
    fs::write(&staging, json).with_context(|| format!("failed to write {}", staging.display()))?;
    fs::rename(&staging, &target)
        .with_context(|| format!("failed to replace {}", target.display()))?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths_include_ryoiki() {
        assert!(path().to_string_lossy().contains("ryoiki"));
        assert!(config_dir().to_string_lossy().contains("ryoiki"));
    }
}
