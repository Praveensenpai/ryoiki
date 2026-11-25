use super::keyboards::{
    docker_keyboard, maintenance_keyboard, poweroff_keyboard, reboot_keyboard, services_keyboard,
    status_keyboard, storage_keyboard, system_keyboard,
};
use super::services::{ContainerInfo, ServiceInfo};
use super::system::{DiskMount, SystemMetrics};
use super::types::InlineKeyboardMarkup;
use crate::modules::media::disk::format_bytes;
use crate::notify::client::escape_html;
use crate::notify::style;

pub fn render_unified_status(
    sys: &SystemMetrics,
    torrent_count: usize,
    active_containers: usize,
) -> (String, InlineKeyboardMarkup) {
    let header = format!(
        "{}\n🟢 <b>{}</b>  ·  {}  ·  ⏱ {}",
        style::view("Command Center"),
        escape_html(&sys.host),
        escape_html(&sys.kernel),
        escape_html(&sys.uptime),
    );
    let telemetry_card = style::card(&format_unified_telemetry(sys));
    let storage_card = style::card(&format!(
        "{}\n{}",
        format_disk_summary(&sys.disks),
        style::row(
            "WORKLOADS",
            &format!("🐳 {active_containers} active  ·  🧲 {torrent_count} tracked")
        )
    ));
    let text = [
        header,
        String::new(),
        format!("📊 {}", style::eyebrow("TELEMETRY")),
        telemetry_card,
        String::new(),
        format!("💾 {}", style::eyebrow("STORAGE & WORKLOADS")),
        storage_card,
    ]
    .join("\n");

    (text, status_keyboard())
}

fn format_unified_telemetry(sys: &SystemMetrics) -> String {
    let (l1, l5, l15) = sys.load_avg;
    let temp = sys
        .cpu_temp
        .map_or_else(|| "N/A".to_string(), |t| format!("{t:.1}°C"));
    let mem_dot = style::status_dot(sys.mem_pct);
    [
        style::row(
            "CPU",
            &format!(
                "{} · load {l1:.2} {l5:.2} {l15:.2}",
                format_cpu_usage(sys.cpu_usage)
            ),
        ),
        style::row(
            "MEM",
            &format!(
                "{mem_dot} <code>{}</code> {}% · {} / {}",
                make_progress_bar(sys.mem_pct, 8),
                sys.mem_pct,
                format_bytes(sys.mem_used),
                format_bytes(sys.mem_total)
            ),
        ),
        style::row(
            "TEMP",
            &format!("{temp}  ·  {}", format_unified_power_status(sys)),
        ),
        style::row(
            "NET",
            &format!(
                "{} · {}",
                escape_html(&sys.tailscale_ip),
                escape_html(&sys.local_ip)
            ),
        ),
    ]
    .join("\n")
}

fn format_unified_power_status(sys: &SystemMetrics) -> String {
    let source = if sys.ac_online { "AC" } else { "battery" };
    let level = sys
        .battery_pct
        .map_or_else(|| "n/a".to_string(), |p| format!("{p}%"));
    sys.charge_limit.map_or_else(
        || format!("{source} · {level}"),
        |cap| format!("{source} · {level} · cap {cap}%"),
    )
}

pub fn render_system_view(sys: &SystemMetrics) -> (String, InlineKeyboardMarkup) {
    let host_card = [
        style::row("HOST", &escape_html(&sys.host)),
        style::row("KERNEL", &escape_html(&sys.kernel)),
        style::row("UPTIME", &escape_html(&sys.uptime)),
    ]
    .join("\n");

    let net_card = [
        style::row("TAILSCALE", &escape_html(&sys.tailscale_ip)),
        style::row("LAN", &escape_html(&sys.local_ip)),
        style::row("PUBLIC", &escape_html(&sys.public_ip)),
    ]
    .join("\n");

    let text = [
        style::view("Host Telemetry"),
        String::new(),
        style::card(&host_card),
        String::new(),
        format!("📊 {}", style::eyebrow("CPU & MEMORY")),
        style::card(&format_system_cpu_mem(sys)),
        String::new(),
        format!("🔋 {}", style::eyebrow("POWER")),
        style::card(&format_system_power(sys)),
        String::new(),
        format!("🌐 {}", style::eyebrow("NETWORK")),
        style::card(&net_card),
    ]
    .join("\n");

    (text, system_keyboard())
}

fn format_system_cpu_mem(sys: &SystemMetrics) -> String {
    let cpu = sys
        .cpu_usage
        .map_or_else(|| "N/A".to_string(), |u| format!("{u:.1}%"));
    let (l1, l5, l15) = sys.load_avg;
    let temp = sys
        .cpu_temp
        .map_or_else(|| "N/A".to_string(), |t| format!("{t:.1}°C"));
    let mem = format!(
        "{} {} / {} ({}%)",
        style::status_dot(sys.mem_pct),
        format_bytes(sys.mem_used),
        format_bytes(sys.mem_total),
        sys.mem_pct
    );
    let swap = format!(
        "{} / {}",
        format_bytes(sys.swap_used),
        format_bytes(sys.swap_total)
    );
    [
        style::row("CPU", &cpu),
        style::row("LOAD", &format!("{l1:.2} {l5:.2} {l15:.2}")),
        style::row("TEMP", &temp),
        style::row("MEM", &mem),
        style::row("SWAP", &swap),
    ]
    .join("\n")
}

fn format_system_power(sys: &SystemMetrics) -> String {
    let level = sys
        .battery_pct
        .map_or_else(|| "N/A".to_string(), |p| format!("{p}%"));
    let state = sys.battery_status.as_deref().unwrap_or("unknown");
    let source = if sys.ac_online {
        "AC online"
    } else {
        "discharging"
    };
    let cap = sys
        .charge_limit
        .map_or_else(|| "none".to_string(), |c| format!("{c}%"));
    [
        style::row("SOURCE", source),
        style::row("BATTERY", &format!("{level} · {state}")),
        style::row("LIMIT", &cap),
    ]
    .join("\n")
}

pub fn render_storage_view(
    disks: &[DiskMount],
    gdrive: Option<&str>,
) -> (String, InlineKeyboardMarkup) {
    let mut lines = vec![style::view("Storage Health"), String::new()];
    if disks.is_empty() {
        lines.push(style::card("No local mounts detected."));
    } else {
        let mut disk_rows = Vec::new();
        for disk in disks {
            let dot = style::status_dot(disk.pct);
            let name = escape_html(&disk.name);
            let path = escape_html(&disk.path);
            let warning = if disk.pct >= 85 { "  ⚠️ high" } else { "" };
            disk_rows.push(format!(
                "{}{} <b>{name}</b>  <code>{path}</code>{warning}",
                style::key("MOUNT"),
                dot
            ));
            disk_rows.push(format!(
                "{}<code>{}</code> {}% · {} / {} · {} free",
                style::key("USED"),
                make_progress_bar(disk.pct, 10),
                disk.pct,
                format_bytes(disk.used),
                format_bytes(disk.total),
                format_bytes(disk.free)
            ));
        }
        lines.push(style::card(&disk_rows.join("\n")));
    }
    if let Some(info) = gdrive {
        lines.push(String::new());
        lines.push(format!("☁️ {}", style::eyebrow("GOOGLE DRIVE")));
        lines.push(style::card(&escape_html(info)));
    }
    (lines.join("\n"), storage_keyboard())
}

pub fn render_docker_view(containers: &[ContainerInfo]) -> (String, InlineKeyboardMarkup) {
    let mut lines = vec![style::view("Docker Containers"), String::new()];
    if containers.is_empty() {
        lines.push(style::card("No containers detected."));
    } else {
        let rows: Vec<String> = containers
            .iter()
            .map(|c| {
                let dot = if c.is_running { "🟢" } else { "⚪" };
                let ports = if c.ports.is_empty() {
                    String::new()
                } else {
                    format!("  <code>{}</code>", escape_html(&c.ports))
                };
                format!(
                    "{dot} <b>{}</b>  <code>{}</code>{ports}",
                    escape_html(&c.name),
                    escape_html(&c.status)
                )
            })
            .collect();
        lines.push(style::card(&rows.join("\n")));
    }
    (lines.join("\n"), docker_keyboard())
}

pub fn render_services_view(services: &[ServiceInfo]) -> (String, InlineKeyboardMarkup) {
    let mut lines = vec![style::view("Managed Services"), String::new()];
    let rows: Vec<String> = services
        .iter()
        .map(|s| {
            let dot = style::service_dot(&s.state);
            let scope = if s.is_user { "user" } else { "system" };
            format!(
                "{dot} <b>{}</b>  <code>{}</code>  <i>{scope}</i>",
                escape_html(&s.name),
                escape_html(&s.state)
            )
        })
        .collect();
    lines.push(style::card(&rows.join("\n")));
    (lines.join("\n"), services_keyboard())
}

pub fn render_maintenance_view() -> (String, InlineKeyboardMarkup) {
    let entries = [
        ("🚀 ORGANIZE", "ingest completed media into Jellyfin"),
        ("🧹 PRUNE", "archive cold or watched media"),
        ("🔄 SYNC", "refresh Jellyfin and cloud"),
        ("🔇 DUBSTRIP", "process queued audio removals"),
        ("🔍 AUDIT", "verify installed CLI tools"),
    ];
    let rows: Vec<String> = entries
        .iter()
        .map(|(label, description)| format!("{} <i>{description}</i>", style::key(label)))
        .collect();
    let text = format!(
        "{}\n\n{}",
        style::view("Server Maintenance"),
        style::card(&rows.join("\n"))
    );
    (text, maintenance_keyboard())
}

pub fn render_reboot_confirm() -> (String, InlineKeyboardMarkup) {
    let body = style::card(&format!(
        "{}\n\n<i>Active connections and tasks will restart.</i>",
        style::accent("⚠️", "CONFIRM SYSTEM REBOOT")
    ));
    let text = format!("{}\n\n{body}", style::view("Reboot"));
    (text, reboot_keyboard())
}

pub fn render_poweroff_confirm() -> (String, InlineKeyboardMarkup) {
    let body = style::card(&format!(
        "{}\n\n<i>Physical or Wake-on-LAN access is required to restart.</i>",
        style::accent("⚠️", "CONFIRM SYSTEM POWER OFF")
    ));
    let text = format!("{}\n\n{body}", style::view("Power Off"));
    (text, poweroff_keyboard())
}

pub fn make_progress_bar(pct: u8, width: usize) -> String {
    let clamped = usize::from(pct.min(100));
    let filled = (clamped * width) / 100;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

fn format_cpu_usage(cpu_usage: Option<f64>) -> String {
    cpu_usage.map_or_else(
        || "N/A".to_string(),
        |usage| {
            let clamped = usage.round().clamp(0.0, 100.0);
            let pct: u8 = format!("{clamped:.0}").parse().unwrap_or(0);
            let dot = style::status_dot(pct);
            format!(
                "{dot} <code>{}</code> {usage:.1}%",
                make_progress_bar(pct, 8)
            )
        },
    )
}

fn format_disk_summary(disks: &[DiskMount]) -> String {
    if disks.is_empty() {
        return "none".to_string();
    }
    disks
        .iter()
        .map(|disk| {
            format!(
                "{}{} <code>{}</code> {}% · {} free",
                style::key(&disk.name),
                style::status_dot(disk.pct),
                make_progress_bar(disk.pct, 6),
                disk.pct,
                format_bytes(disk.free)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_progress_bar_bounds() {
        assert_eq!(make_progress_bar(0, 10), "[░░░░░░░░░░]");
        assert_eq!(make_progress_bar(50, 10), "[█████░░░░░]");
        assert_eq!(make_progress_bar(100, 10), "[██████████]");
        assert_eq!(make_progress_bar(150, 10), "[██████████]");
    }

    #[test]
    fn test_render_unified_status_includes_key_metrics() {
        let sys = SystemMetrics {
            host: "test-box".to_string(),
            kernel: "Linux 6.8".to_string(),
            uptime: "2d 4h".to_string(),
            cpu_usage: Some(25.5),
            load_avg: (0.5, 0.4, 0.3),
            mem_used: 1024 * 1024 * 1024 * 4,
            mem_total: 1024 * 1024 * 1024 * 16,
            mem_pct: 25,
            swap_used: 0,
            swap_total: 1024 * 1024 * 1024 * 8,
            cpu_temp: Some(42.0),
            ac_online: true,
            battery_pct: Some(60),
            battery_status: Some("Charging".to_string()),
            charge_limit: Some(60),
            tailscale_ip: "100.64.0.1".to_string(),
            local_ip: "192.168.1.100".to_string(),
            public_ip: "1.2.3.4".to_string(),
            disks: vec![],
            gdrive_info: None,
        };

        let (text, kb) = render_unified_status(&sys, 3, 2);
        assert!(text.contains("test-box"));
        assert!(text.contains("100.64.0.1"));
        assert!(text.contains("25.5%"));
        assert!(text.contains("<blockquote>"));
        assert!(!text.contains('━'));
        assert!(!text.contains("🌊"));
        assert_eq!(kb.inline_keyboard.len(), 3);
    }
}
