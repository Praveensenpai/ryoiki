use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

use crate::charge_limit::sysfs::write_privileged_file;

const UNIT_PATH: &str = "/etc/systemd/system/ryoiki-auto-poweron.service";
const LOGIND_CONF: &str = "/etc/systemd/logind.conf.d/99-ryoiki-auto-poweron.conf";
const UNIT_NAME: &str = "ryoiki-auto-poweron.service";

/// Installs the shutdown hook and power-button configuration, then sets unit state.
pub fn setup(enable: bool, delay_secs: u32) -> Result<()> {
    write_privileged_file(Path::new(LOGIND_CONF), logind_config())?;
    write_privileged_file(Path::new(UNIT_PATH), &unit_contents(delay_secs))?;
    let _ = Command::new("systemctl").arg("daemon-reload").output();

    let state = if enable { "enable" } else { "disable" };
    systemctl(&[state, UNIT_NAME])?;
    Ok(())
}

/// Reports whether the shutdown hook unit is enabled.
pub fn unit_enabled() -> bool {
    Command::new("systemctl")
        .args(["is-enabled", UNIT_NAME])
        .output()
        .is_ok_and(|out| out.status.success())
}

fn logind_config() -> &'static str {
    "[Login]\nHandlePowerKey=poweroff\n"
}

fn unit_contents(delay_secs: u32) -> String {
    let rtcwake_path = crate::auto_poweron::rtc::rtcwake_path();
    format!(
        "[Unit]\n\
         Description=Ryoiki RTC Auto Power-On\n\
         DefaultDependencies=no\n\
         Before=systemd-poweroff.service poweroff.target\n\n\
         [Service]\n\
         Type=oneshot\n\
         Environment=RYOIKI_DELAY={delay_secs}\n\
         EnvironmentFile=-/run/ryoiki/poweroff-delay.conf\n\
         ExecStart={rtcwake_path} -m no -l -s ${{RYOIKI_DELAY}}\n\
         RemainAfterExit=yes\n\n\
         [Install]\n\
         WantedBy=poweroff.target\n"
    )
}

fn systemctl(args: &[&str]) -> Result<()> {
    let direct = Command::new("systemctl").args(args).output();
    if direct.is_ok_and(|out| out.status.success()) {
        return Ok(());
    }
    let fallback = Command::new("sudo")
        .arg("systemctl")
        .args(args)
        .output()
        .context("Could not run systemctl")?;
    if fallback.status.success() {
        return Ok(());
    }
    bail!(
        "systemctl {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&fallback.stderr).trim()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logind_config_enables_power_key() {
        assert!(logind_config().contains("HandlePowerKey=poweroff"));
    }

    #[test]
    fn test_unit_contents_embeds_delay_and_hook() {
        let unit = unit_contents(300);
        assert!(unit.contains("Environment=RYOIKI_DELAY=300"));
        assert!(unit.contains("EnvironmentFile=-/run/ryoiki/poweroff-delay.conf"));
        assert!(unit.contains("rtcwake -m no -l -s ${RYOIKI_DELAY}"));
        assert!(unit.contains(&format!(
            "ExecStart={} ",
            crate::auto_poweron::rtc::rtcwake_path()
        )));
        assert!(unit.contains("WantedBy=poweroff.target"));
        assert!(unit.contains("Before=systemd-poweroff.service"));
    }
}
