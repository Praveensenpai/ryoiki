use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

use super::sysfs::{read_privileged_file, write_privileged_file};

const ACPI_CALL_PATH: &str = "/proc/acpi/call";
const HP_SBCO_METHOD: &str = "\\_SB.WMID.SBCO";
const HP_GBCO_METHOD: &str = "\\_SB.WMID.GBCO";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HpChargeMode {
    Auto,           // 0x0000 - normal charging
    ForceDischarge, // 0x0200 - discharge while on AC
    InhibitCharge,  // 0x0500 - hold charge, run on AC without charging
}

impl HpChargeMode {
    #[must_use]
    pub fn hex_arg(self) -> &'static str {
        match self {
            Self::Auto => "0x0000",
            Self::ForceDischarge => "0x0200",
            Self::InhibitCharge => "0x0500",
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto (Normal Charging)",
            Self::ForceDischarge => "Force Discharge (Draining to Target)",
            Self::InhibitCharge => "Inhibit Charge (Running on AC Power)",
        }
    }
}

/// Checks if HP ACPI charge mode control is supported on this machine.
pub fn is_hp_acpi_supported() -> bool {
    ensure_acpi_call_loaded();
    if !Path::new(ACPI_CALL_PATH).exists() {
        return false;
    }
    call_acpi(HP_GBCO_METHOD).is_ok_and(|res| !res.contains("Error"))
}

/// Sends an ACPI method call via `/proc/acpi/call` and returns the output.
pub fn call_acpi(call_str: &str) -> Result<String> {
    ensure_acpi_call_loaded();
    let path = Path::new(ACPI_CALL_PATH);
    if !path.exists() {
        bail!("/proc/acpi/call not found. Ensure acpi_call module is loaded.");
    }

    write_privileged_file(path, call_str)
        .with_context(|| format!("Failed to call ACPI method: {call_str}"))?;

    let result = read_privileged_file(path).context("Failed to read ACPI result")?;
    let trimmed = result.trim().to_string();

    if trimmed.contains("Error:") {
        bail!("ACPI call '{call_str}' failed with: {trimmed}");
    }

    Ok(trimmed)
}

/// Queries current charge mode directly from HP firmware via `\_SB.WMID.GBCO`.
pub fn get_hp_charge_mode() -> Result<HpChargeMode> {
    let res = call_acpi(HP_GBCO_METHOD)?;
    if res.contains("0x04") {
        Ok(HpChargeMode::InhibitCharge)
    } else if res.contains("0x02") {
        Ok(HpChargeMode::ForceDischarge)
    } else {
        Ok(HpChargeMode::Auto)
    }
}

/// Sets the charging mode on HP hardware via ACPI method `\_SB.WMID.SBCO`.
pub fn set_hp_charge_mode(mode: HpChargeMode) -> Result<()> {
    if get_hp_charge_mode().ok() == Some(mode) {
        return Ok(());
    }
    let call_str = format!("{HP_SBCO_METHOD} {}", mode.hex_arg());
    call_acpi(&call_str)?;
    Ok(())
}

/// Automatically regulates battery charging between `target_limit - 5%` and `target_limit%`.
pub fn regulate_hp_battery(
    target_limit: u8,
    current_pct: u8,
    ac_online: bool,
) -> Result<HpChargeMode> {
    let min_hysteresis = target_limit.saturating_sub(5);

    if current_pct > target_limit + 2 {
        set_hp_charge_mode(HpChargeMode::ForceDischarge)?;
        Ok(HpChargeMode::ForceDischarge)
    } else if current_pct >= target_limit {
        set_hp_charge_mode(HpChargeMode::InhibitCharge)?;
        Ok(HpChargeMode::InhibitCharge)
    } else if current_pct <= min_hysteresis {
        if ac_online {
            set_hp_charge_mode(HpChargeMode::Auto)?;
        }
        Ok(HpChargeMode::Auto)
    } else {
        // Between hysteresis and target: hold charge, power via AC
        set_hp_charge_mode(HpChargeMode::InhibitCharge)?;
        Ok(HpChargeMode::InhibitCharge)
    }
}

fn ensure_acpi_call_loaded() {
    if !Path::new(ACPI_CALL_PATH).exists() {
        let _ = Command::new("modprobe").arg("acpi_call").output();
        if !Path::new(ACPI_CALL_PATH).exists() {
            let _ = Command::new("sudo")
                .args(["modprobe", "acpi_call"])
                .output();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hp_charge_mode_args() {
        assert_eq!(HpChargeMode::Auto.hex_arg(), "0x0000");
        assert_eq!(HpChargeMode::ForceDischarge.hex_arg(), "0x0200");
        assert_eq!(HpChargeMode::InhibitCharge.hex_arg(), "0x0500");
    }
}
