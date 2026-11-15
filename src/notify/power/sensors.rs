use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

/// Returns whether AC power is connected.
#[must_use]
pub fn is_ac_online() -> bool {
    let base = Path::new("/sys/class/power_supply");
    let Ok(entries) = fs::read_dir(base) else {
        return true;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_mains = fs::read_to_string(path.join("type"))
            .is_ok_and(|k| k.trim().eq_ignore_ascii_case("mains"));
        if is_mains {
            return fs::read_to_string(path.join("online")).is_ok_and(|o| o.trim() == "1");
        }
    }
    false
}

/// Read battery percentage from the first battery found in `/sys/class/power_supply`/.
#[must_use]
pub fn read_battery_percent() -> Option<u8> {
    let base = "/sys/class/power_supply";
    for entry in fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        let type_path = path.join("type");
        let kind = fs::read_to_string(&type_path)
            .map(|s| s.trim().to_lowercase())
            .unwrap_or_default();
        if kind != "battery" {
            continue;
        }
        let cap = fs::read_to_string(path.join("capacity"))
            .ok()
            .and_then(|s| s.trim().parse::<u8>().ok());
        return cap;
    }
    None
}

/// Read battery charge status ("Charging", "Discharging", "Full", …).
#[must_use]
pub fn read_battery_status() -> Option<String> {
    let base = "/sys/class/power_supply";
    for entry in fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        let type_path = path.join("type");
        let kind = fs::read_to_string(&type_path)
            .map(|s| s.trim().to_lowercase())
            .unwrap_or_default();
        if kind != "battery" {
            continue;
        }
        let status = fs::read_to_string(path.join("status"))
            .map(|s| s.trim().to_string())
            .ok();
        return status;
    }
    None
}

/// Read CPU temperature from hwmon or `thermal_zone` sysfs.
#[must_use]
pub fn read_cpu_temp() -> Option<f64> {
    // Try hwmon first (more accurate on most systems)
    if let Some(t) = read_hwmon_temp() {
        return Some(t);
    }
    // Fall back to thermal_zone
    for i in 0..10 {
        let path = format!("/sys/class/thermal/thermal_zone{i}/temp");
        if let Ok(s) = fs::read_to_string(&path) {
            if let Ok(millic) = s.trim().parse::<u32>() {
                return Some(f64::from(millic) / 1000.0);
            }
        }
    }
    None
}

fn read_hwmon_temp() -> Option<f64> {
    let base = "/sys/class/hwmon";
    for entry in fs::read_dir(base).ok()?.flatten() {
        let temp_path = entry.path().join("temp1_input");
        let Ok(raw) = fs::read_to_string(&temp_path) else {
            continue;
        };
        let Ok(millic) = raw.trim().parse::<u32>() else {
            continue;
        };
        let t = f64::from(millic) / 1000.0;
        if (0.0..150.0).contains(&t) {
            return Some(t);
        }
    }
    None
}

/// Read instantaneous CPU usage by sampling /proc/stat twice with a 200 ms gap.
#[must_use]
pub fn read_cpu_usage() -> Option<f64> {
    let (idle1, total1) = parse_cpu_stat()?;
    thread::sleep(Duration::from_millis(200));
    let (idle2, total2) = parse_cpu_stat()?;

    let delta_total = total2.saturating_sub(total1);
    let delta_idle = idle2.saturating_sub(idle1);

    if delta_total == 0 {
        return Some(0.0);
    }
    #[allow(clippy::cast_precision_loss)]
    let usage = 100.0 * (1.0 - delta_idle as f64 / delta_total as f64);
    Some(usage.clamp(0.0, 100.0))
}

fn parse_cpu_stat() -> Option<(u64, u64)> {
    let content = fs::read_to_string("/proc/stat").ok()?;
    let line = content.lines().next()?; // first line: "cpu  ..."
    let mut parts = line.split_whitespace();
    parts.next(); // skip "cpu" label
    let values: Vec<u64> = parts.filter_map(|v| v.parse().ok()).collect();
    if values.len() < 4 {
        return None;
    }
    let idle = values[3]; // idle jiffies
    let total: u64 = values.iter().sum();
    Some((idle, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cpu_stat_reads_proc() {
        // /proc/stat is always present on Linux
        let result = parse_cpu_stat();
        assert!(result.is_some(), "parse_cpu_stat should succeed on Linux");
        if let Some((idle, total)) = result {
            assert!(total >= idle, "total must be >= idle jiffies");
        }
    }

    #[test]
    fn test_cpu_usage_in_range() {
        if let Some(usage) = read_cpu_usage() {
            assert!((0.0..=100.0).contains(&usage));
        }
    }
}
