use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

/// Detects the system vendor from DMI sysfs.
pub fn detect_vendor() -> String {
    fs::read_to_string("/sys/class/dmi/id/sys_vendor")
        .map_or_else(|_| "Unknown".to_string(), |s| s.trim().to_string())
}

/// Detects the system product/model name from DMI sysfs.
pub fn detect_model() -> String {
    fs::read_to_string("/sys/class/dmi/id/product_name")
        .map_or_else(|_| "Unknown Model".to_string(), |s| s.trim().to_string())
}

/// Returns primary config path for saving the target charge limit.
pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    Path::new(&home).join(".config/ryoiki/charge_limit")
}

/// Saves the user's requested charge limit to the config file for the watcher daemon.
pub fn save_charge_limit_config(limit: u8) -> Result<PathBuf> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&path, format!("{limit}\n"))?;
    Ok(path)
}

/// Reads any previously saved charge limit from configuration.
pub fn load_configured_limit() -> Option<u8> {
    let path = config_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(val) = content.trim().parse::<u8>() {
            return Some(val);
        }
    }
    let sys_path = Path::new("/etc/ryoiki/charge_limit");
    if let Ok(content) = fs::read_to_string(sys_path) {
        if let Ok(val) = content.trim().parse::<u8>() {
            return Some(val);
        }
    }
    None
}

/// Displays clear diagnostic info and actionable HP BIOS setup guidance.
pub fn print_unsupported_hardware_guide(vendor: &str, model: &str, target_limit: u8) {
    println!(
        "\n  {} {}",
        "ℹ".yellow().bold(),
        format!("{vendor} {model} hardware detected.").bold()
    );
    println!("  {}\n", "─".repeat(50).dimmed());

    println!(
        "  {} Linux kernel driver does not expose charge thresholds on this laptop.",
        "✖".red().bold()
    );
    println!("    Neither sysfs nor TLP battery care is supported on this model.\n");

    println!(
        "  {} Option 1: Hardware-Enforced Battery Care in HP BIOS (Recommended)",
        "🔋".bold()
    );
    println!("  HP laptops control battery charging directly at the firmware level:");
    println!(
        "    1. Reboot your laptop and press {} to enter BIOS Setup.",
        "F10".cyan().bold()
    );
    println!(
        "    2. Navigate to {} or {}.",
        "Configuration".bold(),
        "Advanced".bold()
    );
    println!(
        "    3. Look for {} and set to {} or {}.",
        "Battery Care Function".cyan().bold(),
        "80%".bold(),
        "50%".bold()
    );
    println!(
        "       (or enable {}).",
        "Adaptive Battery Optimizer".cyan().bold()
    );
    println!(
        "    4. Press {} to save changes and exit.\n",
        "F10".cyan().bold()
    );

    println!(
        "  {} Option 2: Ryoiki Charge Limit Watcher (Software Notification)",
        "🔔".bold()
    );
    println!(
        "    Saved target threshold {}% to {}",
        target_limit.to_string().cyan().bold(),
        config_path().display().to_string().dimmed()
    );
    println!("    The battery watch daemon will alert you when charging reaches this limit:");
    println!("      {} notify battery-watch\n", "ryoiki".cyan().bold());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_path_contains_ryoiki() {
        let p = config_path();
        assert!(p.to_string_lossy().contains("ryoiki"));
    }
}
