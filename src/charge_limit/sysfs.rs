use anyhow::{bail, Context, Result};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

/// Applies the charge limit via sysfs and verifies that the hardware accepted the value.
pub fn apply_sysfs_limit(battery: &Path, limit: u8) -> Result<()> {
    let path = battery.join("charge_control_end_threshold");
    write_sysfs(&path, &limit.to_string())
        .with_context(|| format!("Failed to write to {}", path.display()))?;

    // Verification step: Read back to ensure the kernel/hardware applied the limit
    let current_val = fs::read_to_string(&path)
        .context("Failed to read back threshold to verify")?
        .trim()
        .parse::<u8>()
        .context("Failed to parse read-back threshold value")?;

    if current_val != limit {
        bail!(
            "Hardware rejected limit: wrote {limit}%, but read back {current_val}% from {}",
            path.display()
        );
    }
    Ok(())
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

pub fn battery_name(battery: &Path) -> String {
    battery
        .file_name()
        .map_or_else(|| "BAT0".to_string(), |n| n.to_string_lossy().into_owned())
}

pub fn persist_udev_rule(battery: &Path, limit: u8) -> Result<()> {
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

pub fn persist_systemd_service(battery: &Path, limit: u8) -> Result<()> {
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

pub fn write_privileged_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() {
            let _ = Command::new("sudo")
                .args(["mkdir", "-p", &parent.display().to_string()])
                .output();
        }
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

pub fn read_privileged_file(path: &Path) -> Result<String> {
    if let Ok(content) = fs::read_to_string(path) {
        return Ok(content);
    }
    let output = Command::new("sudo")
        .args(["cat", &path.display().to_string()])
        .output()
        .context("sudo cat failed")?;
    if !output.status.success() {
        bail!("Could not read {} — try running as root", path.display());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_battery_name_extracts_last_segment() {
        let path = PathBuf::from("/sys/class/power_supply/BAT1");
        assert_eq!(battery_name(&path), "BAT1");
    }
}
