use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const MIN_LIMIT: u8 = 20;
const MAX_LIMIT: u8 = 100;

/// Entry point: prompt user, apply limit, persist across reboots.
pub fn run(yes: bool) -> Result<()> {
    let limit = if yes { 60 } else { prompt_limit()? };
    let battery = find_battery_path()?;

    apply_limit(&battery, limit)?;
    persist_udev_rule(&battery, limit)?;
    persist_systemd_service(&battery, limit)?;

    println!(
        "  {} Battery charge limit set to {}% (persists across reboots)",
        "✔".green().bold(),
        limit.to_string().cyan().bold()
    );
    Ok(())
}

// ── Interactive prompt ────────────────────────────────────────────────────────

fn prompt_limit() -> Result<u8> {
    println!("\n  {} Battery Max Charge Limit", "🔋".bold());
    println!("  {}\n", "─".repeat(38).dimmed());
    println!("  Keeping the battery below 80% significantly extends");
    println!("  lifespan on always-plugged servers.\n");
    println!("  {}  100%  — Full capacity (default hardware behaviour)", "[ ]".dimmed());
    println!("  {}   80%  — Recommended for occasional battery use", "[ ]".dimmed());
    println!("  {}   60%  — Optimal for always-plugged servers  {}", "[ ]".dimmed(), "(default)".cyan());
    println!("  {}  Custom — Enter your own value ({}–{}%)\n", "[ ]".dimmed(), MIN_LIMIT, MAX_LIMIT);

    print!("  Select [100 / 80 / 60 / custom] (default 60): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    let trimmed = input.trim().to_lowercase();

    let limit = match trimmed.as_str() {
        "" | "60" => 60,
        "80" => 80,
        "100" => 100,
        "c" | "custom" => prompt_custom_value()?,
        other => {
            if let Ok(n) = other.parse::<u8>() {
                validate_limit(n)?;
                n
            } else {
                bail!("Invalid input: '{other}'. Enter 100, 80, 60, or 'custom'.");
            }
        }
    };

    Ok(limit)
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

// ── Sysfs ─────────────────────────────────────────────────────────────────────

/// Returns the sysfs path of the first battery that supports `charge_control_end_threshold`.
fn find_battery_path() -> Result<PathBuf> {
    let base = Path::new("/sys/class/power_supply");
    for entry in fs::read_dir(base).context("Cannot read /sys/class/power_supply")? {
        let path = entry?.path();
        let kind = fs::read_to_string(path.join("type"))
            .map(|s| s.trim().to_lowercase())
            .unwrap_or_default();
        if kind == "battery" && path.join("charge_control_end_threshold").exists() {
            return Ok(path);
        }
    }
    bail!(
        "No battery with charge_control_end_threshold found.\n  \
        Your hardware or kernel driver may not support this feature."
    )
}

fn apply_limit(battery: &Path, limit: u8) -> Result<()> {
    let threshold_path = battery.join("charge_control_end_threshold");
    write_sysfs(&threshold_path, &limit.to_string())
        .with_context(|| format!("Failed to write to {}", threshold_path.display()))
}

fn write_sysfs(path: &Path, value: &str) -> Result<()> {
    // Try direct write first (running as root), fall back to sudo tee.
    if fs::write(path, value).is_ok() {
        return Ok(());
    }
    let status = Command::new("sudo")
        .args(["tee", &path.display().to_string()])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(value.as_bytes())?;
            }
            child.wait()
        })
        .context("sudo tee failed")?;
    if !status.success() {
        bail!("Could not write '{}' to {} — try running as root", value, path.display());
    }
    Ok(())
}

// ── Persistence ───────────────────────────────────────────────────────────────

fn battery_name(battery: &Path) -> String {
    battery
        .file_name()
        .map_or_else(|| "BAT0".to_string(), |n| n.to_string_lossy().into_owned())
}

fn persist_udev_rule(battery: &Path, limit: u8) -> Result<()> {
    let name = battery_name(battery);
    let rule = format!(
        "# Ryoiki battery charge limit\n\
        ACTION==\"add\", SUBSYSTEM==\"power_supply\", \
        KERNEL==\"{name}\", ATTR{{type}}==\"Battery\", \
        ATTR{{charge_control_end_threshold}}=\"{limit}\"\n"
    );
    let rule_path = Path::new("/etc/udev/rules.d/99-ryoiki-charge-limit.rules");
    write_privileged_file(rule_path, &rule).context("Failed to write udev charge-limit rule")?;
    let _ = Command::new("udevadm").args(["control", "--reload-rules"]).output();
    let _ = Command::new("udevadm").args(["trigger"]).output();
    Ok(())
}

fn persist_systemd_service(battery: &Path, limit: u8) -> Result<()> {
    let threshold_path = battery.join("charge_control_end_threshold");
    let unit = format!(
        "[Unit]\n\
        Description=Ryoiki Battery Charge Limit\n\
        After=multi-user.target\n\n\
        [Service]\n\
        Type=oneshot\n\
        ExecStart=/bin/sh -c 'echo {limit} > {}'\n\
        RemainAfterExit=yes\n\n\
        [Install]\n\
        WantedBy=multi-user.target\n",
        threshold_path.display()
    );
    let svc_path = Path::new("/etc/systemd/system/ryoiki-charge-limit.service");
    write_privileged_file(svc_path, &unit)
        .context("Failed to write ryoiki-charge-limit.service")?;
    let _ = Command::new("systemctl").args(["daemon-reload"]).output();
    let _ = Command::new("systemctl")
        .args(["enable", "--now", "ryoiki-charge-limit.service"])
        .output();
    Ok(())
}

/// Write file directly if possible, otherwise via `sudo tee`.
fn write_privileged_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if fs::write(path, content).is_ok() {
        return Ok(());
    }
    let status = Command::new("sudo")
        .args(["tee", &path.display().to_string()])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(content.as_bytes())?;
            }
            child.wait()
        })
        .context("sudo tee failed")?;
    if !status.success() {
        bail!("Could not write to {} — try running as root", path.display());
    }
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_limit_accepts_valid_range() {
        assert!(validate_limit(20).is_ok());
        assert!(validate_limit(60).is_ok());
        assert!(validate_limit(100).is_ok());
    }

    #[test]
    fn test_validate_limit_rejects_below_min() {
        assert!(validate_limit(19).is_err());
        assert!(validate_limit(0).is_err());
    }

    #[test]
    fn test_validate_limit_rejects_above_max() {
        // u8 can't exceed 255, but validate rejects > 100
        assert!(validate_limit(101).is_err());
    }

    #[test]
    fn test_battery_name_extracts_last_segment() {
        let path = PathBuf::from("/sys/class/power_supply/BAT1");
        assert_eq!(battery_name(&path), "BAT1");
    }
}
