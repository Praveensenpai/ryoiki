use anyhow::{bail, Result};
use std::process::Command;

use crate::notify::client::escape_html;
use crate::notify::style;

pub fn handle_charge_limit(arg: &str) -> String {
    let trimmed = arg.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("status") {
        let current = crate::charge_limit::load_configured_limit().unwrap_or(60);
        let backend = crate::charge_limit::detect_backend().map_or_else(
            |e| format!("Error: {e}"),
            |b| match b {
                crate::charge_limit::Backend::Sysfs(_) => "sysfs (Linux ACPI)".to_string(),
                crate::charge_limit::Backend::HpAcpi => "HP ACPI firmware".to_string(),
                crate::charge_limit::Backend::Tlp => "TLP power management".to_string(),
                crate::charge_limit::Backend::Unsupported { vendor, model } => {
                    format!("Unsupported ({vendor} {model})")
                }
            },
        );
        return format!(
            "{}\n\n{}{}\n{}{}\n\n<i>Adjust with <code>/charge &lt;percentage&gt;</code> (20–100).</i>",
            style::view("Battery Charge Control"),
            style::key("Active Target Limit"),
            format_args!("<b>{current}%</b>"),
            style::key("Hardware Backend"),
            format_args!("<code>{backend}</code>")
        );
    }

    let Ok(val) = trimmed.parse::<u8>() else {
        return format!(
            "{}\n\n<i>Provide a percentage between 20 and 100 (e.g. <code>/charge 80</code>).</i>",
            style::accent("!", "Invalid limit")
        );
    };

    match crate::charge_limit::apply_limit_programmatic(val) {
        Ok(msg) => format!(
            "{}\n\n{}\n{}{}\n{}",
            style::view("Charge Limit"),
            style::accent("✓", "Configured"),
            style::key("TARGET"),
            format_args!("<b>{val}%</b>"),
            format_args!("<i>{msg}</i>")
        ),
        Err(e) => format!(
            "{}\n<code>{e}</code>",
            style::accent("!", "Failed to set charge limit")
        ),
    }
}

pub fn handle_auto_poweron(arg: &str) -> String {
    let arg = arg.trim();
    if arg.is_empty() || arg.eq_ignore_ascii_case("status") {
        return format!(
            "{}\n\n<code>{}</code>\n\n<i>Use <code>/autopower on|off|seconds</code>.</i>",
            style::view("Auto Power-On"),
            crate::auto_poweron::status()
        );
    }

    let Some((enable, delay)) = parse_auto_poweron_arg(arg) else {
        return format!(
            "{}<i>Use /autopower [on|off|status|seconds].</i>",
            style::accent("!", "Invalid argument")
        );
    };
    match crate::auto_poweron::apply_programmatic(enable, delay) {
        Ok(msg) => format!("{}\n\n<i>{msg}</i>", style::accent("✓", "Auto Power-On")),
        Err(e) => format!(
            "{}<code>{e}</code>",
            style::accent("!", "Failed to configure auto power-on")
        ),
    }
}

fn parse_auto_poweron_arg(arg: &str) -> Option<(bool, u32)> {
    let delay = crate::auto_poweron::configured_delay();
    if arg.eq_ignore_ascii_case("on") {
        return Some((true, delay));
    }
    if arg.eq_ignore_ascii_case("off") {
        return Some((false, delay));
    }
    arg.parse::<u32>().ok().map(|secs| (true, secs))
}

pub fn execute_reboot() -> Result<String> {
    let out = Command::new("systemctl")
        .arg("reboot")
        .output()
        .or_else(|_| Command::new("sudo").args(["systemctl", "reboot"]).output())?;

    if out.status.success() {
        Ok(format!(
            "{}\n<i>System is restarting now.</i>",
            style::accent("↻", "Reboot initiated")
        ))
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("Reboot failed: {}", err.trim())
    }
}

pub fn execute_poweroff() -> Result<String> {
    let out = Command::new("systemctl")
        .arg("poweroff")
        .output()
        .or_else(|_| {
            Command::new("sudo")
                .args(["systemctl", "poweroff"])
                .output()
        })?;

    if out.status.success() {
        Ok(format!(
            "{}\n<i>System is powering off now.</i>",
            style::accent("⏻", "Shutdown initiated")
        ))
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("Poweroff failed: {}", err.trim())
    }
}

pub fn handle_self_update() -> String {
    let current_version = env!("CARGO_PKG_VERSION");
    match crate::updater::run_self_update(current_version) {
        Ok(()) => format!(
            "{}\n\n{}\n{}\n\n<i>Binary updated to the latest release. Restarting bot daemon.</i>",
            style::view("Update"),
            style::accent("✓", "Updated"),
            style::row("VERSION", &format!("<code>v{current_version}</code>"))
        ),
        Err(e) => format!(
            "{}\n\n{}\n{}\n{}",
            style::view("Update"),
            style::accent("·", "Update check"),
            style::row("VERSION", &format!("<code>v{current_version}</code>")),
            style::row("STATUS", &escape_html(&e.to_string()))
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_charge_limit_invalid() {
        let res = handle_charge_limit("abc");
        assert!(res.contains("Invalid limit"));

        let res_negative = handle_charge_limit("-5");
        assert!(res_negative.contains("Invalid limit"));
    }

    #[test]
    fn test_handle_charge_limit_status_queries_backend() {
        let res = handle_charge_limit("status");
        assert!(res.contains("Battery Charge Control"));
        assert!(res.contains("Active Target Limit"));
    }

    #[test]
    fn test_parse_auto_poweron_arg() {
        let delay = crate::auto_poweron::configured_delay();
        assert_eq!(parse_auto_poweron_arg("on"), Some((true, delay)));
        assert_eq!(parse_auto_poweron_arg("off"), Some((false, delay)));
        assert_eq!(parse_auto_poweron_arg("600"), Some((true, 600)));
        assert_eq!(parse_auto_poweron_arg("abc"), None);
    }

    #[test]
    fn test_handle_auto_poweron_rejects_garbage() {
        assert!(handle_auto_poweron("garbage").contains("Invalid argument"));
    }
}
