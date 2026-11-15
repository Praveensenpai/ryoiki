pub mod model;
pub mod store;

pub use model::{Settings, DEFAULT_ALERTS, MAX_CHARGE, MAX_POWEROFF, MIN_CHARGE, MIN_POWEROFF};
pub use store::{config_dir, load, load_or_default, path as settings_path, save};

/// Effective charge limit: persisted settings first, then the legacy file.
#[must_use]
pub fn effective_charge_limit() -> u8 {
    load_or_default().battery.max_charge_percent
}
