pub mod model;
pub mod store;

pub use model::{
    validate_prune, Settings, DEFAULT_ALERTS, MAX_CHARGE, MAX_POWEROFF, MAX_PRUNE, MIN_CHARGE,
    MIN_POWEROFF, MIN_PRUNE, MIN_PRUNE_GAP, MIN_PRUNE_TARGET,
};
pub use store::{config_dir, load, load_or_default, path as settings_path, save};

/// Effective charge limit: persisted settings first, then the legacy file.
#[must_use]
pub fn effective_charge_limit() -> u8 {
    load_or_default().battery.max_charge_percent
}
