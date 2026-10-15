use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

use super::sysfs::{read_privileged_file, write_privileged_file};

const ACPI_CALL_PATH: &str = "/proc/acpi/call";
const HP_SBCO_METHOD: &str = "\\_SB.WMID.SBCO";
const HP_GBCO_METHOD: &str = "\\_SB.WMID.GBCO";
const HP_BST0_REG: &str = "\\_SB.PCI0.LPCB.BST0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HpChargeMode {
    Auto, // 0x0000 - normal charging
    /// `0x0200`. Decoded from hardware but never commanded: its ACPI handler
    /// clears the AC-present bit (`RPWR &= 0xFE`) with no restore path.
    ForceDischarge,
    InhibitCharge, // 0x0500 - hold charge, run on AC without charging
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

/// Decodes the live charge mode from the embedded-controller status byte `BST0`.
///
/// Neither `GBCO` nor `BST0` is a commanded-mode echo. `GBCO`'s `0x05` branch
/// fires whenever the "on AC" bit (`0x40`) is set, so it reports `0x05` for
/// every mode while the charger is plugged in. `BST0` is the EC *status* byte:
/// its bits describe the current power path, not the last `SBCO` argument.
///
/// Live values observed on this HP (board 849A):
/// - `0x00` -> on battery, idle
/// - `0x40` -> AC present, idle (at limit / not charging)
/// - `0x41` -> AC present, discharging
/// - `0x42` -> AC present, charging
/// - `0x80` -> `SBCO 0x0100` applied
/// - `0x90` -> charge inhibited (`SBCO 0x0500`)
/// - `0xd0` -> charge inhibited while AC idle (live at the 60% limit)
/// - `0xc0`/`0xc8` -> `SBCO 0x0300` applied
///
/// Only bit `0x10` is a charge-inhibit signal. Bit `0x01` merely means the pack
/// is discharging, which is a normal power-path state and never a charge
/// command, so it must not be read as `ForceDischarge`. Anything without
/// `0x10` maps to `Auto` so the daemon keeps (re-)asserting normal charging.
fn parse_bst0(reply: &str) -> HpChargeMode {
    // `/proc/acpi/call` returns the value NUL-terminated and possibly with
    // trailing junk (e.g. "0x90\0"). `str::trim` does not strip NUL, so drop
    // the terminator, remove an optional `0x` prefix once, then take only the
    // leading hex digits. Otherwise `from_str_radix` fails and every mode
    // decodes as Auto.
    let cleaned = reply.trim_matches(|c: char| c.is_whitespace() || c == '\0');
    let body = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
        .unwrap_or(cleaned);
    let hex: String = body.chars().take_while(char::is_ascii_hexdigit).collect();
    let value = u32::from_str_radix(&hex, 16).unwrap_or(0);

    if value & 0x10 != 0 {
        HpChargeMode::InhibitCharge
    } else {
        HpChargeMode::Auto
    }
}

/// Reads the live charge mode from the EC status register via `\_SB.PCI0.LPCB.BST0`.
pub fn get_hp_charge_mode() -> Result<HpChargeMode> {
    let res = call_acpi(HP_BST0_REG)?;
    Ok(parse_bst0(&res))
}

/// Sets the charging mode on HP hardware via ACPI method `\_SB.WMID.SBCO`.
///
/// `ForceDischarge` is rejected outright: its `SBCO` handler runs
/// `\_SB.PCI0.LPCB.RPWR &= 0xFE`, clearing the AC-present bit with no firmware
/// path to restore it, so the EC and ACPI then report the adapter as absent.
pub fn set_hp_charge_mode(mode: HpChargeMode) -> Result<()> {
    if mode == HpChargeMode::ForceDischarge {
        bail!("refusing to send ForceDischarge (0x0200): it clears the AC-present bit");
    }
    if get_hp_charge_mode().ok() == Some(mode) {
        return Ok(());
    }
    let call_str = format!("{HP_SBCO_METHOD} {}", mode.hex_arg());
    call_acpi(&call_str)?;
    Ok(())
}

/// Picks the charge mode for `current_pct` given the live `current` mode.
///
/// The pack is kept inside a ±2% dead band around `target_limit` (58–62% for a
/// 60% limit). Below the lower edge charging resumes; at or above the upper edge
/// charging is held; inside the band the mode is left unchanged so the EC does
/// not flap between charge and hold. `ForceDischarge` is never chosen.
fn desired_mode(target_limit: u8, current_pct: u8, current: HpChargeMode) -> HpChargeMode {
    let lower = target_limit.saturating_sub(2);
    let upper = target_limit.saturating_add(2).min(100);

    if current_pct <= lower {
        HpChargeMode::Auto
    } else if current_pct >= upper || current == HpChargeMode::InhibitCharge {
        HpChargeMode::InhibitCharge
    } else {
        HpChargeMode::Auto
    }
}

/// Automatically regulates battery charging around `target_limit`.
///
/// The pack is held inside the ±2% band around the limit. When the adapter is
/// offline there is nothing to regulate, so the EC is left untouched and its
/// current mode is reported back.
pub fn regulate_hp_battery(
    target_limit: u8,
    current_pct: u8,
    ac_online: bool,
) -> Result<HpChargeMode> {
    let current = get_hp_charge_mode().unwrap_or(HpChargeMode::Auto);
    if !ac_online {
        return Ok(current);
    }

    let desired = desired_mode(target_limit, current_pct, current);
    set_hp_charge_mode(desired)?;
    Ok(get_hp_charge_mode().unwrap_or(desired))
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

    #[test]
    fn test_parse_bst0_maps_hardware_modes() {
        // Values observed live on the HP EC (board 849A): the discharge bit
        // (0x01) is a power-path status, not a charge command.
        assert_eq!(parse_bst0("0x00"), HpChargeMode::Auto);
        assert_eq!(parse_bst0("0x40"), HpChargeMode::Auto);
        assert_eq!(parse_bst0("0x41"), HpChargeMode::Auto);
        assert_eq!(parse_bst0("0x80"), HpChargeMode::Auto);
        assert_eq!(parse_bst0("0xc0"), HpChargeMode::Auto);
        assert_eq!(parse_bst0("0xc8"), HpChargeMode::Auto);
        assert_eq!(parse_bst0("0x90"), HpChargeMode::InhibitCharge);
        assert_eq!(parse_bst0("0xd0"), HpChargeMode::InhibitCharge);
    }

    #[test]
    fn test_parse_bst0_tolerates_nul_terminator() {
        // `/proc/acpi/call` returns e.g. "0x90\0" verbatim.
        assert_eq!(parse_bst0("0x90\0"), HpChargeMode::InhibitCharge);
        assert_eq!(parse_bst0("0x40\0"), HpChargeMode::Auto);
        assert_eq!(parse_bst0("0x41\0"), HpChargeMode::Auto);
        assert_eq!(parse_bst0("0x90\0\n"), HpChargeMode::InhibitCharge);
        assert_eq!(parse_bst0("0x90\0trailing"), HpChargeMode::InhibitCharge);
    }

    #[test]
    fn test_parse_bst0_malformed_defaults_to_auto() {
        assert_eq!(parse_bst0(""), HpChargeMode::Auto);
        assert_eq!(parse_bst0("garbage"), HpChargeMode::Auto);
        assert_eq!(parse_bst0("0X00"), HpChargeMode::Auto);
    }

    #[test]
    fn test_desired_mode_holds_band_around_limit() {
        // Limit 60 -> band 58..62.
        assert_eq!(desired_mode(60, 0, HpChargeMode::Auto), HpChargeMode::Auto);
        assert_eq!(desired_mode(60, 57, HpChargeMode::Auto), HpChargeMode::Auto);
        assert_eq!(
            desired_mode(60, 58, HpChargeMode::InhibitCharge),
            HpChargeMode::Auto
        );
        // Inside the band the live mode is left alone (no flapping).
        assert_eq!(desired_mode(60, 60, HpChargeMode::Auto), HpChargeMode::Auto);
        assert_eq!(
            desired_mode(60, 60, HpChargeMode::InhibitCharge),
            HpChargeMode::InhibitCharge
        );
        assert_eq!(
            desired_mode(60, 62, HpChargeMode::Auto),
            HpChargeMode::InhibitCharge
        );
        assert_eq!(
            desired_mode(60, 80, HpChargeMode::Auto),
            HpChargeMode::InhibitCharge
        );
        // Saturating edges for the minimum and maximum limits.
        assert_eq!(desired_mode(1, 0, HpChargeMode::Auto), HpChargeMode::Auto);
        assert_eq!(
            desired_mode(100, 100, HpChargeMode::Auto),
            HpChargeMode::InhibitCharge
        );
    }

    #[test]
    fn test_desired_mode_never_force_discharges() {
        for target in [20u8, 50, 60, 80, 100] {
            for pct in 0..=100u8 {
                for current in [HpChargeMode::Auto, HpChargeMode::InhibitCharge] {
                    assert_ne!(
                        desired_mode(target, pct, current),
                        HpChargeMode::ForceDischarge
                    );
                }
            }
        }
    }

    #[test]
    fn test_set_hp_charge_mode_rejects_force_discharge() {
        assert!(set_hp_charge_mode(HpChargeMode::ForceDischarge).is_err());
    }
}
