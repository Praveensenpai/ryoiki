use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

const WAKEALARM: &str = "/sys/class/rtc/rtc0/wakealarm";

/// Parses a wakealarm sysfs value into a Unix timestamp.
fn parse_wakealarm(raw: &str) -> Option<u64> {
    raw.trim().parse::<u64>().ok()
}

/// Returns the armed RTC wake time as a Unix timestamp when one is set.
pub fn read_wakealarm() -> Option<u64> {
    fs::read_to_string(WAKEALARM)
        .ok()
        .and_then(|raw| parse_wakealarm(&raw))
}

/// Reports whether this host can arm an RTC alarm.
pub fn supported() -> bool {
    Path::new(WAKEALARM).exists() && command_exists("rtcwake")
}

/// Arms the RTC wake alarm without suspending the running system.
pub fn set_alarm(delay_secs: u32) -> Result<String> {
    let value = delay_secs.to_string();
    let direct = Command::new("rtcwake")
        .args(["-m", "no", "-l", "-s", &value])
        .output();
    if direct.is_ok_and(|out| out.status.success()) {
        return Ok(format!("RTC alarm armed for {delay_secs}s"));
    }
    let fallback = Command::new("sudo")
        .args(["rtcwake", "-m", "no", "-l", "-s", &value])
        .output()
        .context("Could not execute rtcwake; install util-linux")?;
    if fallback.status.success() {
        return Ok(format!("RTC alarm armed for {delay_secs}s"));
    }
    bail!(
        "rtcwake failed: {}",
        String::from_utf8_lossy(&fallback.stderr).trim()
    )
}

fn command_exists(name: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {name}")])
        .output()
        .is_ok_and(|out| out.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_exists_uses_shell_lookup() {
        assert!(command_exists("sh"));
        assert!(!command_exists("definitely-not-a-real-binary-ryoiki"));
    }

    #[test]
    fn test_parse_wakealarm() {
        assert_eq!(parse_wakealarm("1795016560\n"), Some(1_795_016_560));
        assert_eq!(parse_wakealarm("0\n"), Some(0));
        assert_eq!(parse_wakealarm("garbage"), None);
        assert_eq!(parse_wakealarm("  "), None);
    }
}
