use crate::runner;
use colored::Colorize;

/// Prints the final styled setup summary box with individual timings and service highlights.
pub fn print_summary(
    module_ids: &[String],
    total_duration: std::time::Duration,
    timings: &[(String, std::time::Duration)],
) {
    let total_str = runner::format_duration(total_duration);
    let title_line = format!("✨ 領域 (Ryoiki) Server Setup Complete in {total_str}!");
    let border_len = (title_line.chars().count() + 6).max(52);
    let border = "─".repeat(border_len);

    println!("  {}", border.dimmed());
    println!("  {}", title_line.green().bold());
    println!("  {}", border.dimmed());

    if !timings.is_empty() {
        println!("  • {}", "Timings:".dimmed());
        for (name, dur) in timings {
            let dur_str = runner::format_duration(*dur);
            println!("    {} {:<26} {}", "•".dimmed(), name, dur_str.cyan());
        }
        println!();
    }

    print_module_highlights(module_ids);

    println!(
        "  • {} Log saved to ~/.local/state/ryoiki/install.log",
        "Debug:   ".dimmed()
    );
    println!("  {}\n", border.dimmed());
}

fn print_module_highlights(module_ids: &[String]) {
    print_cli_highlights(module_ids);
    print_infra_highlights(module_ids);
}

fn print_cli_highlights(module_ids: &[String]) {
    if module_ids
        .iter()
        .any(|m| m == "dotfiles" || m == "cli_tools" || m == "trash")
    {
        println!(
            "  • {} eza (ls) • bat (cat) • zoxide (cd) • toss (rm)",
            "Aliases: ".dimmed()
        );
    }
    if module_ids.iter().any(|m| m == "dev_runtimes") {
        println!(
            "  • {} Go • Rust (cargo) • Python (uv) • JavaScript (bun)",
            "Runtimes:".dimmed()
        );
    }
}

fn print_infra_highlights(module_ids: &[String]) {
    if module_ids.iter().any(|m| m == "security") {
        println!("  • {} UFW (22, 80, 443) active", "Security:".dimmed());
    }
    if module_ids.iter().any(|m| m == "docker") {
        println!(
            "  • {} Docker Engine & Docker Compose plugin active",
            "Docker:  ".dimmed()
        );
    }
    if module_ids.iter().any(|m| m == "jellyfin") {
        println!(
            "  • {} Jellyfin live on port 8096 (Intel QSV enabled)",
            "Media:   ".dimmed()
        );
    }
    if module_ids.iter().any(|m| m == "torrent") {
        println!(
            "  • {} qBittorrent live on port 6881 (Web UI enabled)",
            "Torrent: ".dimmed()
        );
    }
    if module_ids.iter().any(|m| m == "tailscale") {
        println!(
            "  • {} Tailscale MagicDNS active (connect via hostname)",
            "Mesh VPN:".dimmed()
        );
    }
    if module_ids.iter().any(|m| m == "charge_limit") {
        println!(
            "  • {} Battery charge limit active (persists across reboots)",
            "Battery: ".dimmed()
        );
    }
    if module_ids.iter().any(|m| m == "rclone") {
        println!(
            "  • {} Google Drive mounted at ~/gdrive (systemd auto-start)",
            "Storage: ".dimmed()
        );
    }
}
