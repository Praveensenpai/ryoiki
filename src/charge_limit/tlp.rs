use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;

use super::guide::detect_vendor;
use super::sysfs::write_privileged_file;
use super::MIN_LIMIT;

/// Known vendors with official hardware support in TLP Battery Care plugins.
const TLP_SUPPORTED_VENDORS: &[&str] = &[
    "lenovo",
    "thinkpad",
    "ibm",
    "asus",
    "dell",
    "huawei",
    "lg",
    "msi",
    "samsung",
    "sony",
    "system76",
    "toshiba",
    "framework",
];

/// Verifies whether TLP actually provides battery threshold control for this hardware.
pub fn is_tlp_supported_for_hardware() -> bool {
    let vendor = detect_vendor().to_lowercase();
    if vendor.contains("hp") || vendor.contains("hewlett") {
        return false;
    }

    if command_exists("tlp") {
        let status = Command::new("sh")
            .args([
                "-c",
                ". /usr/share/tlp/tlp-func-base 2>/dev/null; \
                 select_batdrv 2>/dev/null; \
                 [ \"$_bm_thresh\" != \"none\" ] && [ -n \"$_bm_thresh\" ]",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if let Ok(s) = status {
            return s.success();
        }
    }

    TLP_SUPPORTED_VENDORS.iter().any(|&v| vendor.contains(v))
}

pub fn apply_tlp_limit(limit: u8) -> Result<()> {
    if !is_tlp_supported_for_hardware() {
        bail!("TLP does not support battery charge thresholds on this hardware.");
    }
    ensure_tlp_installed()?;
    configure_tlp_thresholds(limit)?;
    reload_tlp()
}

pub fn ensure_tlp_installed() -> Result<()> {
    if command_exists("tlp") {
        return Ok(());
    }
    println!(
        "  {} Installing TLP for battery charge control...",
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

pub fn configure_tlp_thresholds(stop: u8) -> Result<()> {
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

pub fn reload_tlp() -> Result<()> {
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
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
