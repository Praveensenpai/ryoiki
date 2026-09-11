use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const MIN_LIMIT: u8 = 20;
const MAX_LIMIT: u8 = 100;

enum Backend {
    Sysfs(PathBuf),
    Tlp,
}

fn detect_backend() -> Result<Backend> {
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
    Ok(Backend::Tlp)
}

pub fn run(yes: bool) -> Result<()> {
    let backend = detect_backend()?;
    let limit = if yes { 60 } else { prompt_limit()? };

    match &backend {
        Backend::Sysfs(battery) => {
            apply_sysfs_limit(battery, limit)?;
            persist_udev_rule(battery, limit)?;
            persist_systemd_service(battery, limit)?;
            println!(
                "  {} Battery charge limit set to {}% via sysfs (persists across reboots)",
                "✔".green().bold(),
                limit.to_string().cyan().bold()
            );
        }
        Backend::Tlp => {
            apply_tlp_limit(limit)?;
            println!(
                "  {} Battery charge limit set to {}% via TLP (persists across reboots)",
                "✔".green().bold(),
                limit.to_string().cyan().bold()
            );
        }
    }
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

// ── Sysfs backend ─────────────────────────────────────────────────────────────

fn apply_sysfs_limit(battery: &Path, limit: u8) -> Result<()> {
    let path = battery.join("charge_control_end_threshold");
    write_sysfs(&path, &limit.to_string())
        .with_context(|| format!("Failed to write to {}", path.display()))
}

fn write_sysfs(path: &Path, value: &str) -> Result<()> {
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
        bail!(
            "Could not write '{}' to {} — try running as root",
            value,
            path.display()
        );
    }
    Ok(())
}

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
    write_privileged_file(
        Path::new("/etc/udev/rules.d/99-ryoiki-charge-limit.rules"),
        &rule,
    )
    .context("Failed to write udev charge-limit rule")?;
    let _ = Command::new("udevadm")
        .args(["control", "--reload-rules"])
        .output();
    let _ = Command::new("udevadm").args(["trigger"]).output();
    Ok(())
}

fn persist_systemd_service(battery: &Path, limit: u8) -> Result<()> {
    let threshold_path = battery.join("charge_control_end_threshold");
    let unit = format!(
        "[Unit]\nDescription=Ryoiki Battery Charge Limit\nAfter=multi-user.target\n\n\
        [Service]\nType=oneshot\nExecStart=/bin/sh -c 'echo {limit} > {}'\nRemainAfterExit=yes\n\n\
        [Install]\nWantedBy=multi-user.target\n",
        threshold_path.display()
    );
    write_privileged_file(
        Path::new("/etc/systemd/system/ryoiki-charge-limit.service"),
        &unit,
    )
    .context("Failed to write ryoiki-charge-limit.service")?;
    let _ = Command::new("systemctl").args(["daemon-reload"]).output();
    let _ = Command::new("systemctl")
        .args(["enable", "--now", "ryoiki-charge-limit.service"])
        .output();
    Ok(())
}

// ── TLP backend ───────────────────────────────────────────────────────────────

fn apply_tlp_limit(limit: u8) -> Result<()> {
    ensure_tlp_installed()?;
    configure_tlp_thresholds(limit)?;
    reload_tlp()
}

fn ensure_tlp_installed() -> Result<()> {
    if command_exists("tlp") {
        return Ok(());
    }
    println!(
        "  {} Installing TLP for HP battery charge control...",
        "→".cyan()
    );
    let status = Command::new("sudo")
        .args(["apt-get", "install", "-y", "tlp"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("Failed to run apt-get install tlp")?;
    if !status.success() {
        bail!("Failed to install TLP. Run: sudo apt-get install -y tlp");
    }
    Ok(())
}

fn configure_tlp_thresholds(stop: u8) -> Result<()> {
    let start = stop.saturating_sub(10).max(MIN_LIMIT);
    let existing = fs::read_to_string("/etc/tlp.conf").unwrap_or_default();
    let updated = update_tlp_conf(&existing, start, stop);
    write_privileged_file(Path::new("/etc/tlp.conf"), &updated)
        .context("Failed to write /etc/tlp.conf")
}

pub fn update_tlp_conf(conf: &str, start: u8, stop: u8) -> String {
    let start_key = "START_CHARGE_THRESH_BAT0";
    let stop_key = "STOP_CHARGE_THRESH_BAT0";
    let mut lines: Vec<String> = conf.lines().map(String::from).collect();
    let (mut found_start, mut found_stop) = (false, false);

    for line in &mut lines {
        if line.starts_with(start_key) || line.starts_with(&format!("#{start_key}")) {
            *line = format!("{start_key}={start}");
            found_start = true;
        } else if line.starts_with(stop_key) || line.starts_with(&format!("#{stop_key}")) {
            *line = format!("{stop_key}={stop}");
            found_stop = true;
        }
    }

    if !found_start {
        lines.push(format!("{start_key}={start}"));
    }
    if !found_stop {
        lines.push(format!("{stop_key}={stop}"));
    }
    lines.join("\n") + "\n"
}

fn reload_tlp() -> Result<()> {
    let _ = Command::new("sudo")
        .args(["systemctl", "enable", "--now", "tlp.service"])
        .output();
    let status = Command::new("sudo")
        .args(["tlp", "start"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("Failed to run tlp start")?;
    if !status.success() {
        bail!("TLP configured but 'tlp start' failed. Reboot to apply.");
    }
    Ok(())
}

fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ── Privileged file write ─────────────────────────────────────────────────────

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
        bail!(
            "Could not write to {} — try running as root",
            path.display()
        );
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
    fn test_battery_name_extracts_last_segment() {
        let path = PathBuf::from("/sys/class/power_supply/BAT1");
        assert_eq!(battery_name(&path), "BAT1");
    }

    #[test]
    fn test_update_tlp_conf_appends_when_missing() {
        let result = update_tlp_conf("# TLP config\nTLP_ENABLE=1\n", 50, 60);
        assert!(result.contains("START_CHARGE_THRESH_BAT0=50"));
        assert!(result.contains("STOP_CHARGE_THRESH_BAT0=60"));
    }

    #[test]
    fn test_update_tlp_conf_replaces_existing() {
        let conf = "START_CHARGE_THRESH_BAT0=75\nSTOP_CHARGE_THRESH_BAT0=80\n";
        let result = update_tlp_conf(conf, 50, 60);
        assert!(result.contains("START_CHARGE_THRESH_BAT0=50"));
        assert!(result.contains("STOP_CHARGE_THRESH_BAT0=60"));
        assert_eq!(result.matches("START_CHARGE_THRESH_BAT0").count(), 1);
    }

    #[test]
    fn test_update_tlp_conf_uncomments_commented_keys() {
        let conf = "#START_CHARGE_THRESH_BAT0=75\n#STOP_CHARGE_THRESH_BAT0=80\n";
        let result = update_tlp_conf(conf, 50, 60);
        assert!(result.contains("START_CHARGE_THRESH_BAT0=50"));
        assert!(result.contains("STOP_CHARGE_THRESH_BAT0=60"));
    }

    #[test]
    fn test_thresholds() {
        // stop - 10 = start; saturates at MIN_LIMIT
        assert_eq!(60_u8.saturating_sub(10).max(MIN_LIMIT), 50);
        assert_eq!(25_u8.saturating_sub(10).max(MIN_LIMIT), MIN_LIMIT);
    }

    #[test]
    fn test_parse_limit_input() {
        assert_eq!(parse_limit_input("").unwrap(), 60);
        assert_eq!(parse_limit_input("80").unwrap(), 80);
        assert_eq!(parse_limit_input("100").unwrap(), 100);
        assert!(parse_limit_input("abc").is_err());
        assert!(parse_limit_input("19").is_err());
        assert!(parse_limit_input("101").is_err());
    }
}
