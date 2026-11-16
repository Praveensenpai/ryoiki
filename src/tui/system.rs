use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use crate::notify::power::{
    is_ac_online, read_battery_percent, read_battery_status, read_cpu_temp, read_cpu_usage,
    sample_cpu_usage,
};
use crate::notify::system::{
    format_usage, get_disk_stats, get_kernel, get_memory_stats, get_uptime,
};

const BATTERY_UNIT: &str = "ryoiki-battery-watch.service";
const LOADING: &str = "checking...";

/// Point-in-time system metrics shown on the dashboard.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub host: String,
    pub kernel: String,
    pub uptime: String,
    pub cpu: String,
    pub memory: String,
    pub disk: String,
    pub battery_percent: Option<u8>,
    pub battery_status: String,
    pub ac_online: bool,
    pub charge_backend: String,
    pub watcher_state: String,
    pub watcher_enabled: bool,
    pub prune_timer_active: bool,
}

impl Snapshot {
    /// Compact AC/battery label for status rows.
    #[must_use]
    pub fn power_label(&self) -> &'static str {
        if self.ac_online {
            "AC connected"
        } else {
            "on battery"
        }
    }
}

/// One row in the Services tab.
#[derive(Debug, Clone)]
pub struct ServiceRow {
    pub name: String,
    pub description: String,
    pub state: String,
    pub is_user: bool,
    pub container: bool,
}

/// Collects managed systemd units plus any Docker containers.
#[must_use]
pub fn collect_services() -> Vec<ServiceRow> {
    let mut rows: Vec<ServiceRow> = crate::bot::services::get_managed_services()
        .into_iter()
        .map(|svc| ServiceRow {
            name: svc.name,
            description: svc.description,
            state: svc.state,
            is_user: svc.is_user,
            container: false,
        })
        .collect();

    if let Ok(containers) = crate::bot::services::get_docker_containers() {
        rows.extend(containers.into_iter().map(|container| ServiceRow {
            name: container.name,
            description: "Docker container".to_string(),
            state: container.status,
            is_user: false,
            container: true,
        }));
    }
    rows
}

/// Samples CPU usage between refreshes without sleeping on the UI thread.
#[derive(Debug, Default)]
pub struct CpuSampler {
    previous: Option<(u64, u64)>,
}

impl CpuSampler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns usage since the previous sample, or `None` for the first call.
    pub fn sample(&mut self) -> Option<f64> {
        sample_cpu_usage(&mut self.previous)
    }
}

/// Fast snapshot used to paint the first frame before the worker finishes.
///
/// The charge backend and CPU usage are deliberately left pending; probing them
/// synchronously is what made the interface feel unresponsive.
#[must_use]
pub fn collect_fast() -> Snapshot {
    let mut snapshot = collect_inner(None, None);
    snapshot.cpu = "sampling...".to_string();
    snapshot
}

/// Collects a fresh snapshot with blocking probes. Used by non-interactive output.
#[must_use]
pub fn collect() -> Snapshot {
    let backend = backend_label();
    let cpu = read_cpu_usage();
    collect_inner(cpu, Some(backend))
}

/// Collects a fresh snapshot for the background refresh worker.
#[must_use]
pub fn collect_for_worker(sampler: &mut CpuSampler) -> Snapshot {
    let backend = backend_label();
    let cpu = sampler.sample();
    collect_inner(cpu, Some(backend))
}

fn collect_inner(cpu_usage: Option<f64>, backend: Option<String>) -> Snapshot {
    let (watcher_state, watcher_enabled) = watcher_state();
    Snapshot {
        host: hostname(),
        kernel: get_kernel(),
        uptime: get_uptime(),
        cpu: cpu_label(cpu_usage),
        memory: get_memory_stats().map_or_else(|| "n/a".to_string(), |(u, t)| format_usage(u, t)),
        disk: get_disk_stats().map_or_else(|| "n/a".to_string(), |(u, t)| format_usage(u, t)),
        battery_percent: read_battery_percent(),
        battery_status: read_battery_status().unwrap_or_else(|| "unknown".to_string()),
        ac_online: is_ac_online(),
        charge_backend: backend.unwrap_or_else(|| LOADING.to_string()),
        watcher_state,
        watcher_enabled,
        prune_timer_active: crate::modules::media::prune_timer::is_timer_active(),
    }
}

fn hostname() -> String {
    fs::read_to_string("/proc/sys/kernel/hostname")
        .map_or_else(|_| "unknown".to_string(), |s| s.trim().to_string())
}

fn cpu_label(usage: Option<f64>) -> String {
    let temp = read_cpu_temp().map_or_else(|| "n/a".to_string(), |t| format!("{t:.1}°C"));
    let usage = usage.map_or_else(|| "n/a".to_string(), |u| format!("{u:.0}%"));
    format!("{usage}  {temp}")
}

fn backend_label() -> String {
    static LABEL: OnceLock<String> = OnceLock::new();
    LABEL.get_or_init(compute_backend_label).clone()
}

/// Display-only label that avoids the firmware probe.
///
/// The real `detect_backend()` calls HP ACPI `GBCO`, which can sleep in the
/// kernel for about a second. Actions still use `detect_backend()`; this label
/// only needs to describe the likely backend without blocking the interface.
fn compute_backend_label() -> String {
    let base = Path::new("/sys/class/power_supply");
    let Ok(entries) = fs::read_dir(base) else {
        return "no battery".to_string();
    };
    let mut has_battery = false;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_battery = fs::read_to_string(path.join("type"))
            .is_ok_and(|kind| kind.trim().eq_ignore_ascii_case("battery"));
        if !is_battery {
            continue;
        }
        has_battery = true;
        if path.join("charge_control_end_threshold").exists() {
            let name = path.file_name().map_or_else(
                || "battery".to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            return format!("sysfs ({name})");
        }
    }
    if !has_battery {
        return "no battery".to_string();
    }

    let vendor = crate::charge_limit::guide::detect_vendor();
    let vendor_lower = vendor.to_lowercase();
    if vendor_lower.contains("hp") || vendor_lower.contains("hewlett") {
        return if Path::new("/proc/acpi/call").exists() {
            "HP ACPI firmware".to_string()
        } else {
            "HP ACPI (acpi_call not loaded)".to_string()
        };
    }
    if crate::charge_limit::tlp::is_tlp_supported_for_hardware() {
        return "TLP".to_string();
    }
    format!(
        "unsupported ({vendor} {})",
        crate::charge_limit::guide::detect_model()
    )
}

fn watcher_state() -> (String, bool) {
    let active = systemctl(&["is-active", BATTERY_UNIT]);
    let enabled = systemctl(&["is-enabled", BATTERY_UNIT]);
    let state = if active.is_empty() {
        "unknown".to_string()
    } else {
        active
    };
    (state, enabled == "enabled")
}

fn systemctl(args: &[&str]) -> String {
    Command::new("systemctl").args(args).output().map_or_else(
        |_| String::new(),
        |out| String::from_utf8_lossy(&out.stdout).trim().to_string(),
    )
}

/// Best-effort heuristic that tells the Provision tab whether a module is present.
#[must_use]
pub fn module_installed(id: &str) -> bool {
    match id {
        "git_ssh" => home_path(".ssh/id_ed25519").exists(),
        "essentials" => has_all(&["git", "tmux", "nvim", "gh"]),
        "cli_tools" => has_all(&["eza", "bat", "zoxide", "fzf"]),
        "dev_runtimes" => has_all(&["go", "cargo", "uv", "bun"]),
        "timezone" => Path::new("/etc/timezone").exists(),
        "security" => command_exists("ufw"),
        "docker" => command_exists("docker"),
        "rclone" => command_exists("rclone"),
        "prompt" => command_exists("starship"),
        "trash" => command_exists("toss"),
        "dubstrip" => command_exists("dubstrip"),
        "seedr" => command_exists("seedr-dl"),
        "tailscale" => command_exists("tailscale"),
        "dotfiles" => home_path(".tmux.conf").exists(),
        "charge_limit" => crate::settings::settings_path().exists(),
        "headless_audio" => home_path(".asoundrc").exists(),
        _ => false,
    }
}

fn has_all(commands: &[&str]) -> bool {
    commands.iter().all(|cmd| command_exists(cmd))
}

fn command_exists(name: &str) -> bool {
    crate::runner::Runner::command_exists(name)
}

fn home_path(suffix: &str) -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    Path::new(&home).join(suffix)
}
