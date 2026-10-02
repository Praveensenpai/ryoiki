use super::keyboards::{
    docker_keyboard, maintenance_keyboard, poweroff_keyboard, reboot_keyboard, services_keyboard,
    status_keyboard, storage_keyboard, system_keyboard,
};
use super::services::{ContainerInfo, ServiceInfo};
use super::system::{DiskMount, SystemMetrics};
use super::types::InlineKeyboardMarkup;
use crate::modules::media::disk::format_bytes;

pub fn render_unified_status(
    sys: &SystemMetrics,
    torrent_count: usize,
    active_containers: usize,
) -> (String, InlineKeyboardMarkup) {
    let cpu_str = format_cpu_usage(sys.cpu_usage);
    let (l1, l5, l15) = sys.load_avg;
    let mem_bar = make_progress_bar(sys.mem_pct, 8);
    let mem_used_str = format_bytes(sys.mem_used);
    let mem_total_str = format_bytes(sys.mem_total);
    let temp_str = sys
        .cpu_temp
        .map_or_else(|| "N/A".to_string(), |t| format!("{t:.1}°C"));

    let power_icon = if sys.ac_online {
        "🔌 AC"
    } else {
        "🔋 Battery"
    };
    let batt_str = sys
        .battery_pct
        .map_or_else(|| "None".to_string(), |p| format!("{p}%"));
    let charge_cap_str = sys
        .charge_limit
        .map_or_else(String::new, |c| format!(" (⚡ {c}% Cap)"));

    let disk_summary = format_disk_summary(&sys.disks);
    let tailscale_display = if sys.tailscale_ip.is_empty() {
        "none"
    } else {
        &sys.tailscale_ip
    };

    let text = format!(
        "🌊 <b>領域 RYOIKI • Command Center</b>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        🖥 <b>{}</b> • {} • Up {}\n\
        🌐 <code>{}</code> (Tailscale) • <code>{}</code> (LAN)\n\n\
        📊 <b>System Load:</b>\n\
        CPU:  {} (Load: {:.2}, {:.2}, {:.2})\n\
        RAM:  <code>{}</code> {}% ({} / {})\n\
        Temp: {} • Power: {} [{}]{}\n\n\
        💾 <b>Storage:</b>\n\
        {}\n\n\
        🐳 <b>Containers & Services:</b>\n\
        Docker: <b>{}</b> active container(s)\n\
        Torrents: <b>{}</b> active/tracked\n\
        ━━━━━━━━━━━━━━━━━━━━━━━",
        sys.host,
        sys.kernel,
        sys.uptime,
        tailscale_display,
        sys.local_ip,
        cpu_str,
        l1,
        l5,
        l15,
        mem_bar,
        sys.mem_pct,
        mem_used_str,
        mem_total_str,
        temp_str,
        power_icon,
        batt_str,
        charge_cap_str,
        disk_summary,
        active_containers,
        torrent_count,
    );

    (text, status_keyboard())
}

pub fn render_system_view(sys: &SystemMetrics) -> (String, InlineKeyboardMarkup) {
    let cpu_str = sys
        .cpu_usage
        .map_or_else(|| "N/A".to_string(), |u| format!("{u:.1}%"));
    let (l1, l5, l15) = sys.load_avg;
    let mem_used_str = format_bytes(sys.mem_used);
    let mem_total_str = format_bytes(sys.mem_total);
    let swap_used_str = format_bytes(sys.swap_used);
    let swap_total_str = format_bytes(sys.swap_total);
    let temp_str = sys
        .cpu_temp
        .map_or_else(|| "N/A".to_string(), |t| format!("{t:.1} °C"));

    let batt_pct_str = sys
        .battery_pct
        .map_or_else(|| "N/A".to_string(), |p| format!("{p}%"));
    let batt_st = sys.battery_status.as_deref().unwrap_or("Unknown");
    let limit_str = sys
        .charge_limit
        .map_or_else(|| "None".to_string(), |c| format!("{c}%"));

    let text = format!(
        "📊 <b>領域 RYOIKI • Host Telemetry</b>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        🖥 <b>Hostname:</b> {}\n\
        🐧 <b>Kernel:</b>   {}\n\
        ⏱ <b>Uptime:</b>   {}\n\n\
        ⚙️ <b>CPU & Memory:</b>\n\
        • CPU Utilization: <b>{}</b>\n\
        • Load Average:    <b>{:.2}, {:.2}, {:.2}</b>\n\
        • CPU Temperature: <b>{}</b>\n\
        • Physical RAM:    <b>{} / {}</b> ({}%)\n\
        • Swap Space:      <b>{} / {}</b>\n\n\
        🔋 <b>Power & Battery:</b>\n\
        • AC Power:        <b>{}</b>\n\
        • Charge Level:    <b>{}</b> ({})\n\
        • Active Limit:    <b>{}</b>\n\n\
        🌐 <b>Networking:</b>\n\
        • Tailscale IPv4:  <code>{}</code>\n\
        • Local LAN IP:    <code>{}</code>\n\
        • Public IP:       <code>{}</code>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━",
        sys.host,
        sys.kernel,
        sys.uptime,
        cpu_str,
        l1,
        l5,
        l15,
        temp_str,
        mem_used_str,
        mem_total_str,
        sys.mem_pct,
        swap_used_str,
        swap_total_str,
        if sys.ac_online {
            "Online (Plugged)"
        } else {
            "Offline (Discharging)"
        },
        batt_pct_str,
        batt_st,
        limit_str,
        if sys.tailscale_ip.is_empty() {
            "none"
        } else {
            &sys.tailscale_ip
        },
        sys.local_ip,
        sys.public_ip,
    );

    (text, system_keyboard())
}

pub fn render_storage_view(
    disks: &[DiskMount],
    gdrive: Option<&str>,
) -> (String, InlineKeyboardMarkup) {
    let mut lines = vec![
        "💾 <b>領域 RYOIKI • Storage Health</b>".to_string(),
        "━━━━━━━━━━━━━━━━━━━━━━━".to_string(),
    ];

    for d in disks {
        let bar = make_progress_bar(d.pct, 10);
        let u_str = format_bytes(d.used);
        let f_str = format_bytes(d.free);
        let t_str = format_bytes(d.total);
        let badge = if d.pct >= 85 { " ⚠️ High" } else { "" };
        lines.push(format!(
            "📦 <b>{}</b> (<code>{}</code>){}\n<code>{}</code> {}%\nUsed: {} | Free: {} | Total: {}\n",
            d.name, d.path, badge, bar, d.pct, u_str, f_str, t_str
        ));
    }

    if let Some(gd) = gdrive {
        lines.push(format!("☁️ <b>Google Drive (gdrive:):</b>\n{gd}\n"));
    }

    lines.push("━━━━━━━━━━━━━━━━━━━━━━━".to_string());
    (lines.join("\n"), storage_keyboard())
}

pub fn render_docker_view(containers: &[ContainerInfo]) -> (String, InlineKeyboardMarkup) {
    let mut lines = vec![
        "🐳 <b>領域 RYOIKI • Docker Containers</b>".to_string(),
        "━━━━━━━━━━━━━━━━━━━━━━━".to_string(),
    ];

    if containers.is_empty() {
        lines.push("No Docker containers detected.".to_string());
    } else {
        for c in containers {
            let icon = if c.is_running { "🟢" } else { "🔴" };
            let port_info = if c.ports.is_empty() {
                String::new()
            } else {
                format!("\nPorts: <code>{}</code>", c.ports)
            };
            lines.push(format!(
                "{icon} <b>{}</b>\nStatus: <code>{}</code>{}",
                c.name, c.status, port_info
            ));
        }
    }

    lines.push("━━━━━━━━━━━━━━━━━━━━━━━".to_string());
    (lines.join("\n"), docker_keyboard())
}

pub fn render_services_view(services: &[ServiceInfo]) -> (String, InlineKeyboardMarkup) {
    let mut lines = vec![
        "🛠 <b>領域 RYOIKI • Managed Services</b>".to_string(),
        "━━━━━━━━━━━━━━━━━━━━━━━".to_string(),
    ];

    for s in services {
        let icon = match s.state.as_str() {
            "active" => "🟢",
            "inactive" => "⚪",
            _ => "🔴",
        };
        let scope = if s.is_user { "user" } else { "system" };
        lines.push(format!(
            "{icon} <b>{}</b> [{scope}]\n<code>{}</code> • {}",
            s.name, s.state, s.description
        ));
    }

    lines.push("━━━━━━━━━━━━━━━━━━━━━━━".to_string());
    (lines.join("\n"), services_keyboard())
}

pub fn render_maintenance_view() -> (String, InlineKeyboardMarkup) {
    let text = "⚙️ <b>領域 RYOIKI • Server Maintenance</b>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        Select a maintenance workflow to execute:\n\n\
        🎬 <b>Organize:</b> Ingest completed media into Jellyfin\n\
        🧹 <b>Prune:</b> Evict cold/watched media to Google Drive\n\
        🔄 <b>Sync:</b> Trigger Jellyfin refresh & cloud sync\n\
        🗡️ <b>Dubstrip:</b> Process queued audio track removals\n\
        🔍 <b>Audit:</b> Verify installed CLI runtimes & tools\n\
        🔋 <b>Charge Limit:</b> Set battery stop threshold\n\
        ━━━━━━━━━━━━━━━━━━━━━━━";

    (text.to_string(), maintenance_keyboard())
}

pub fn render_reboot_confirm() -> (String, InlineKeyboardMarkup) {
    let text = "⚠️ <b>CONFIRM SYSTEM REBOOT</b>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        Are you sure you want to reboot the server?\n\
        All active network connections and tasks will restart.\n\
        ━━━━━━━━━━━━━━━━━━━━━━━";

    (text.to_string(), reboot_keyboard())
}

pub fn render_poweroff_confirm() -> (String, InlineKeyboardMarkup) {
    let text = "🔴 <b>CONFIRM SYSTEM POWER OFF</b>\n\
        ━━━━━━━━━━━━━━━━━━━━━━━\n\
        Are you sure you want to completely shut down the server?\n\
        Physical or Wake-on-LAN access is required to power it back on.\n\
        ━━━━━━━━━━━━━━━━━━━━━━━";

    (text.to_string(), poweroff_keyboard())
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
        |u| {
            let u_int = if u >= 100.0 {
                100
            } else if u <= 0.0 {
                0
            } else {
                let mut p = 0;
                while f64::from(p + 1) <= u {
                    p += 1;
                }
                p
            };
            let bar = make_progress_bar(u_int, 8);
            format!("<code>{bar}</code> {u:.1}%")
        },
    )
}

fn format_disk_summary(disks: &[DiskMount]) -> String {
    disks
        .iter()
        .map(|d| {
            let bar = make_progress_bar(d.pct, 6);
            let u_str = format_bytes(d.used);
            let t_str = format_bytes(d.total);
            format!(
                "• <b>{}</b>: <code>{}</code> {}% ({} / {})",
                d.name, bar, d.pct, u_str, t_str
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
        let empty = make_progress_bar(0, 10);
        assert_eq!(empty, "[░░░░░░░░░░]");

        let half = make_progress_bar(50, 10);
        assert_eq!(half, "[█████░░░░░]");

        let full = make_progress_bar(100, 10);
        assert_eq!(full, "[██████████]");

        let overflow = make_progress_bar(150, 10);
        assert_eq!(overflow, "[██████████]");
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
        assert_eq!(kb.inline_keyboard.len(), 4);
    }
}
