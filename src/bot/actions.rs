use anyhow::{bail, Result};
use std::process::Command;

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
            "🔋 <b>Battery Charge Control</b>\n\
            ━━━━━━━━━━━━━━━━━━━━━━━\n\
            Active Target Limit: <b>{current}%</b>\n\
            Hardware Backend: <code>{backend}</code>\n\n\
            To adjust, send: <code>/charge &lt;percentage&gt;</code> (e.g. <code>/charge 80</code>)"
        );
    }

    let Ok(val) = trimmed.parse::<u8>() else {
        return "❌ <b>Invalid limit:</b> Please provide a percentage between 20 and 100 (e.g. <code>/charge 80</code>).".to_string();
    };

    match crate::charge_limit::apply_limit_programmatic(val) {
        Ok(msg) => format!(
            "⚡ <b>Charge Limit Configured</b>\n\
            ━━━━━━━━━━━━━━━━━━━━━━━\n\
            ✔ {msg}\n\
            New Target: <b>{val}%</b>"
        ),
        Err(e) => format!("❌ <b>Failed to set charge limit:</b>\n<code>{e}</code>"),
    }
}

pub fn execute_reboot() -> Result<String> {
    let out = Command::new("systemctl")
        .arg("reboot")
        .output()
        .or_else(|_| Command::new("sudo").args(["systemctl", "reboot"]).output())?;

    if out.status.success() {
        Ok("🔄 <b>Reboot initiated.</b> System is restarting now...".to_string())
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
        Ok("⏻ <b>Shutdown initiated.</b> System is powering off now...".to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("Poweroff failed: {}", err.trim())
    }
}

pub fn handle_self_update() -> String {
    let current_version = env!("CARGO_PKG_VERSION");
    match crate::updater::run_self_update(current_version) {
        Ok(()) => format!(
            "✨ <b>Ryoiki Updated</b>\n\
            ━━━━━━━━━━━━━━━━━━━━━━━\n\
            Current version: <code>v{current_version}</code>\n\
            Binary updated to latest release. Restarting bot daemon..."
        ),
        Err(e) => format!(
            "ℹ️ <b>Ryoiki Update Check</b>\n\
            ━━━━━━━━━━━━━━━━━━━━━━━\n\
            Current version: <code>v{current_version}</code>\n\
            Status: {e}"
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
}
