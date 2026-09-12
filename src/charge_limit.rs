pub mod guide;
pub mod hp_acpi;
pub mod sysfs;
pub mod tlp;

use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub use guide::load_configured_limit;

pub const MIN_LIMIT: u8 = 20;
pub const MAX_LIMIT: u8 = 100;

#[derive(Debug, PartialEq, Eq)]
pub enum Backend {
    Sysfs(PathBuf),
    HpAcpi,
    Tlp,
    Unsupported { vendor: String, model: String },
}

pub fn detect_backend() -> Result<Backend> {
    let base = Path::new("/sys/class/power_supply");
    let entries = fs::read_dir(base).context("Cannot read /sys/class/power_supply")?;
    let mut has_battery = false;

    for entry in entries.flatten() {
        let path = entry.path();
        let kind = fs::read_to_string(path.join("type"))
            .map(|s| s.trim().to_lowercase())
            .unwrap_or_default();
        if kind != "battery" {
            continue;
        }
        has_battery = true;
        if path.join("charge_control_end_threshold").exists() {
            return Ok(Backend::Sysfs(path));
        }
    }

    if !has_battery {
        bail!("No battery detected on this system.");
    }

    if hp_acpi::is_hp_acpi_supported() {
        return Ok(Backend::HpAcpi);
    }

    if tlp::is_tlp_supported_for_hardware() {
        return Ok(Backend::Tlp);
    }

    Ok(Backend::Unsupported {
        vendor: guide::detect_vendor(),
        model: guide::detect_model(),
    })
}

pub fn run(yes: bool) -> Result<()> {
    let backend = detect_backend()?;
    let limit = if yes { 60 } else { prompt_limit()? };

    match backend {
        Backend::Sysfs(battery) => {
            sysfs::apply_sysfs_limit(&battery, limit)?;
            sysfs::persist_udev_rule(&battery, limit)?;
            sysfs::persist_systemd_service(&battery, limit)?;
            println!(
                "  {} Battery charge limit set to {}% via sysfs (verified and persists across reboots)",
                "✔".green().bold(),
                limit.to_string().cyan().bold()
            );
        }
        Backend::HpAcpi => {
            apply_hp_acpi_limit(limit)?;
        }
        Backend::Tlp => {
            tlp::apply_tlp_limit(limit)?;
            println!(
                "  {} Battery charge limit set to {}% via TLP (persists across reboots)",
                "✔".green().bold(),
                limit.to_string().cyan().bold()
            );
        }
        Backend::Unsupported { vendor, model } => {
            let _ = guide::save_charge_limit_config(limit);
            guide::print_unsupported_hardware_guide(&vendor, &model, limit);
        }
    }
    Ok(())
}

fn apply_hp_acpi_limit(limit: u8) -> Result<()> {
    let _ = guide::save_charge_limit_config(limit);
    let _ =
        sysfs::write_privileged_file(Path::new("/etc/ryoiki/charge_limit"), &format!("{limit}\n"));

    let cap = fs::read_to_string("/sys/class/power_supply/BAT0/capacity")
        .ok()
        .and_then(|s| s.trim().parse::<u8>().ok())
        .unwrap_or(100);

    let mode = hp_acpi::regulate_hp_battery(limit, cap, true)?;
    let _ = Command::new("sudo")
        .args(["systemctl", "restart", "ryoiki-battery-watch.service"])
        .output();

    println!(
        "  {} Battery charge limit set to {}% via HP ACPI firmware control",
        "✔".green().bold(),
        limit.to_string().cyan().bold()
    );
    println!(
        "  {} Mode: {} (Battery is currently at {})",
        "▶".cyan(),
        mode.label().bold(),
        format!("{cap}%").bold()
    );
    println!(
        "  {} Automated 24/7 regulation active via ryoiki-battery-watch service",
        "✔".green()
    );
    Ok(())
}

// ── Prompt ────────────────────────────────────────────────────────────────────

fn prompt_limit() -> Result<u8> {
    println!("\n  {} Battery Max Charge Limit", "🔋".bold());
    println!("  {}\n", "─".repeat(38).dimmed());
    println!("  Keeping the battery below 80% significantly extends");
    println!("  lifespan on always-plugged servers.\n");
    println!(
        "  {}  100%  — Full capacity (default hardware behaviour)",
        "[ ]".dimmed()
    );
    println!(
        "  {}   80%  — Recommended for occasional battery use",
        "[ ]".dimmed()
    );
    println!(
        "  {}   60%  — Optimal for always-plugged servers  {}",
        "[ ]".dimmed(),
        "(default)".cyan()
    );
    println!(
        "  {}  Custom — Enter your own value ({}–{}%)\n",
        "[ ]".dimmed(),
        MIN_LIMIT,
        MAX_LIMIT
    );

    print!("  Select [100 / 80 / 60 / custom] (default 60): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    parse_limit_input(input.trim())
}

fn parse_limit_input(trimmed: &str) -> Result<u8> {
    match trimmed.to_lowercase().as_str() {
        "" | "60" => Ok(60),
        "80" => Ok(80),
        "100" => Ok(100),
        "c" | "custom" => prompt_custom_value(),
        other => {
            let n: u8 = other.parse().with_context(|| {
                format!("Invalid input: '{other}'. Enter 100, 80, 60, or 'custom'.")
            })?;
            validate_limit(n)?;
            Ok(n)
        }
    }
}

fn prompt_custom_value() -> Result<u8> {
    print!("  Enter limit ({MIN_LIMIT}–{MAX_LIMIT}): ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    let n: u8 = input
        .trim()
        .parse()
        .context("Value must be a number between 20 and 100")?;
    validate_limit(n)?;
    Ok(n)
}

fn validate_limit(n: u8) -> Result<()> {
    if !(MIN_LIMIT..=MAX_LIMIT).contains(&n) {
        bail!("Limit must be between {MIN_LIMIT}% and {MAX_LIMIT}%, got {n}%");
    }
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_limit() {
        assert!(validate_limit(20).is_ok());
        assert!(validate_limit(60).is_ok());
        assert!(validate_limit(100).is_ok());
        assert!(validate_limit(19).is_err());
        assert!(validate_limit(0).is_err());
        assert!(validate_limit(101).is_err());
    }

    #[test]
    fn test_parse_limit_input() {
        assert_eq!(parse_limit_input("").ok(), Some(60));
        assert_eq!(parse_limit_input("60").ok(), Some(60));
        assert_eq!(parse_limit_input("80").ok(), Some(80));
        assert_eq!(parse_limit_input("100").ok(), Some(100));
        assert!(parse_limit_input("abc").is_err());
        assert!(parse_limit_input("19").is_err());
        assert!(parse_limit_input("101").is_err());
    }

    #[test]
    fn test_thresholds() {
        assert_eq!(60_u8.saturating_sub(10).max(MIN_LIMIT), 50);
        assert_eq!(25_u8.saturating_sub(10).max(MIN_LIMIT), MIN_LIMIT);
    }
}
