use std::ffi::CString;
use std::fs;
use std::mem::MaybeUninit;
use std::path::Path;
use std::process::Command;

use crate::notify::config::TelegramConfig;
use crate::notify::power::{
    is_ac_online, read_battery_percent, read_battery_status, read_cpu_temp, read_cpu_usage,
};
use crate::notify::system::{
    get_hostname, get_kernel, get_memory_stats, get_public_ip, get_tailscale_ip, get_uptime,
};

#[derive(Debug, Clone)]
pub struct DiskMount {
    pub name: String,
    pub path: String,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub pct: u8,
}

#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub host: String,
    pub kernel: String,
    pub uptime: String,
    pub cpu_usage: Option<f64>,
    pub load_avg: (f32, f32, f32),
    pub mem_used: u64,
    pub mem_total: u64,
    pub mem_pct: u8,
    pub swap_used: u64,
    pub swap_total: u64,
    pub cpu_temp: Option<f64>,
    pub ac_online: bool,
    pub battery_pct: Option<u8>,
    pub battery_status: Option<String>,
    pub charge_limit: Option<u8>,
    pub tailscale_ip: String,
    pub local_ip: String,
    pub public_ip: String,
    pub disks: Vec<DiskMount>,
    pub gdrive_info: Option<String>,
}

pub fn collect_system_metrics(config: &TelegramConfig) -> SystemMetrics {
    let host = get_hostname(config);
    let kernel = get_kernel();
    let uptime = get_uptime();
    let cpu_usage = read_cpu_usage();
    let load_avg = get_load_averages();
    let (mem_used, mem_total) = get_memory_stats().unwrap_or((0, 0));
    let mem_pct = mem_used
        .checked_mul(100)
        .and_then(|u| u.checked_div(mem_total))
        .and_then(|p| u8::try_from(p).ok())
        .unwrap_or(0);
    let (swap_used, swap_total) = get_swap_stats();
    let cpu_temp = read_cpu_temp();
    let ac_online = is_ac_online();
    let battery_pct = read_battery_percent();
    let battery_status = read_battery_status();
    let charge_limit = crate::charge_limit::load_configured_limit();

    let tailscale_ip = get_tailscale_ip();
    let local_ip = get_local_ip();
    let public_ip = get_public_ip();

    let disks = collect_disk_mounts();
    let gdrive_info = collect_gdrive_summary();

    SystemMetrics {
        host,
        kernel,
        uptime,
        cpu_usage,
        load_avg,
        mem_used,
        mem_total,
        mem_pct,
        swap_used,
        swap_total,
        cpu_temp,
        ac_online,
        battery_pct,
        battery_status,
        charge_limit,
        tailscale_ip,
        local_ip,
        public_ip,
        disks,
        gdrive_info,
    }
}

pub fn get_load_averages() -> (f32, f32, f32) {
    fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| {
            let mut parts = s.split_whitespace();
            let one: f32 = parts.next()?.parse().ok()?;
            let five: f32 = parts.next()?.parse().ok()?;
            let fifteen: f32 = parts.next()?.parse().ok()?;
            Some((one, five, fifteen))
        })
        .unwrap_or((0.0, 0.0, 0.0))
}

pub fn get_swap_stats() -> (u64, u64) {
    let Ok(content) = fs::read_to_string("/proc/meminfo") else {
        return (0, 0);
    };
    let mut total_kb: Option<u64> = None;
    let mut free_kb: Option<u64> = None;
    for line in content.lines() {
        if line.starts_with("SwapTotal:") {
            total_kb = line.split_whitespace().nth(1).and_then(|v| v.parse().ok());
        } else if line.starts_with("SwapFree:") {
            free_kb = line.split_whitespace().nth(1).and_then(|v| v.parse().ok());
        }
    }
    let total = total_kb.unwrap_or(0).saturating_mul(1024);
    let free = free_kb.unwrap_or(0).saturating_mul(1024);
    let used = total.saturating_sub(free);
    (used, total)
}

pub fn get_local_ip() -> String {
    Command::new("hostname")
        .arg("-I")
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.split_whitespace().next().map(ToString::to_string)
        })
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

pub fn get_disk_info(path: &str) -> Option<(u64, u64, u64)> {
    let c_path = CString::new(path).ok()?;
    let mut stat = MaybeUninit::<libc::statvfs>::uninit();
    let res = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if res == 0 {
        let s = unsafe { stat.assume_init() };
        let total = s.f_blocks.saturating_mul(s.f_frsize);
        let free = s.f_bavail.saturating_mul(s.f_frsize);
        let used = total.saturating_sub(free);
        Some((total, used, free))
    } else {
        None
    }
}

fn collect_disk_mounts() -> Vec<DiskMount> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let torrents = format!("{home}/torrents");
    let jellyfin = format!("{home}/jellyfin/media");

    let candidates = [
        ("NVMe Root (/)", "/"),
        ("Home (/home)", "/home"),
        ("Torrents Storage", torrents.as_str()),
        ("Jellyfin Library", jellyfin.as_str()),
    ];

    let mut mounts = Vec::new();
    let mut seen_totals = Vec::new();

    for (label, path) in candidates {
        if !Path::new(path).exists() {
            continue;
        }
        if let Some((total, used, free)) = get_disk_info(path) {
            if total == 0 {
                continue;
            }
            // Avoid duplicate reporting if multiple paths are on same partition
            if seen_totals.contains(&(total, free)) && path != "/" {
                continue;
            }
            seen_totals.push((total, free));
            let pct = used
                .checked_mul(100)
                .and_then(|u| u.checked_div(total))
                .and_then(|p| u8::try_from(p).ok())
                .unwrap_or(0);
            mounts.push(DiskMount {
                name: label.to_string(),
                path: path.to_string(),
                total,
                used,
                free,
                pct,
            });
        }
    }
    mounts
}

fn collect_gdrive_summary() -> Option<String> {
    let out = Command::new("rclone")
        .args(["about", "gdrive:"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let mut used = String::new();
    let mut total = String::new();
    for line in s.lines() {
        if line.starts_with("Used:") {
            used = line.strip_prefix("Used:").unwrap_or("").trim().to_string();
        } else if line.starts_with("Total:") {
            total = line.strip_prefix("Total:").unwrap_or("").trim().to_string();
        }
    }
    if used.is_empty() {
        None
    } else if total.is_empty() {
        Some(format!("{used} used"))
    } else {
        Some(format!("{used} / {total}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_load_averages_returns_valid_tuple() {
        let (one, five, fifteen) = get_load_averages();
        assert!(one >= 0.0);
        assert!(five >= 0.0);
        assert!(fifteen >= 0.0);
    }

    #[test]
    fn test_get_swap_stats_does_not_panic() {
        let (used, total) = get_swap_stats();
        assert!(used <= total || total == 0);
    }

    #[test]
    fn test_get_disk_info_root() {
        let res = get_disk_info("/");
        assert!(res.is_some());
        if let Some((total, used, free)) = res {
            assert!(total > 0);
            assert!(used <= total);
            assert!(free <= total);
        }
    }

    #[test]
    fn test_get_local_ip_non_empty() {
        let ip = get_local_ip();
        assert!(!ip.is_empty());
    }
}
