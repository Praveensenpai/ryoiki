pub mod model;
pub mod store;

pub use model::{
    validate_prune, AutoPoweronSettings, Settings, DEFAULT_ALERTS, MAX_AUTO_POWERON_DELAY,
    MAX_CHARGE, MAX_POWEROFF, MAX_PRUNE, MIN_AUTO_POWERON_DELAY, MIN_CHARGE, MIN_POWEROFF,
    MIN_PRUNE, MIN_PRUNE_GAP, MIN_PRUNE_TARGET,
};
pub use store::{config_dir, load, load_or_default, path as settings_path, save};

/// Effective charge limit: 100 when the ceiling is turned off, otherwise the
/// persisted `battery.max_charge_percent`, then the legacy file.
#[must_use]
pub fn effective_charge_limit() -> u8 {
    let settings = load_or_default();
    if settings.battery.max_charge_enabled {
        settings.battery.max_charge_percent
    } else {
        100
    }
}
