use crate::runner::Runner;
use anyhow::Result;
use colored::Colorize;
use std::io::{self, BufRead, Write};

/// Provisions Tailscale mesh VPN, enabling `MagicDNS` hostname SSH access without static IP.
pub fn setup(runner: &mut Runner, non_interactive: bool) -> Result<()> {
    install_tailscale(runner)?;
    enable_service(runner)?;

    if runner.dry_run {
        println!(
            "  {} [dry-run] Tailscale up with MagicDNS and SSH",
            "•".dimmed()
        );
        return Ok(());
    }

    if is_authenticated() {
        show_active_info();
        return Ok(());
    }

    if non_interactive {
        show_manual_instructions();
    } else {
        prompt_and_connect()?;
    }

    Ok(())
}

fn install_tailscale(runner: &mut Runner) -> Result<()> {
    if Runner::command_exists("tailscale") {
        println!("  {} Tailscale client is already installed", "✔".green());
        return Ok(());
    }

    runner.exec_silent(
        "Installing Tailscale client...",
        "sh",
        &["-c", "curl -fsSL https://tailscale.com/install.sh | sh"],
    )?;
    Ok(())
}

fn enable_service(runner: &mut Runner) -> Result<()> {
    runner.exec_silent(
        "Enabling tailscaled service...",
        "systemctl",
        &["enable", "--now", "tailscaled"],
    )?;
    Ok(())
}

fn is_authenticated() -> bool {
    let output = std::process::Command::new("tailscale")
        .arg("status")
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            !text.contains("Logged out") && !text.contains("Tailscale is stopped")
        }
        _ => false,
    }
}

fn show_active_info() {
    let host = get_machine_hostname();
    println!(
        "  {} Tailscale is connected with MagicDNS active",
        "✔".green().bold()
    );
    if !host.is_empty() {
        println!(
            "    Connect from any device on your tailnet: {}",
            format!("ssh <user>@{host}").cyan().bold()
        );
    }
}

fn show_manual_instructions() {
    println!("  {} Tailscale installed & service active.", "✔".green());
    println!(
        "    To authenticate with MagicDNS and SSH, run: {}",
        "sudo tailscale up --ssh".cyan().bold()
    );
}

fn prompt_and_connect() -> Result<()> {
    print!("  Connect to Tailscale network now? [Y/n]: ");
    io::stdout().flush()?;
    let mut choice = String::new();
    io::stdin().lock().read_line(&mut choice)?;

    if choice.trim().eq_ignore_ascii_case("n") {
        show_manual_instructions();
        return Ok(());
    }

    println!(
        "\n  {} Opening Tailscale authentication...",
        "▶".cyan().bold()
    );
    let status = std::process::Command::new("tailscale")
        .args(["up", "--ssh"])
        .status();

    handle_connect_status(&status);
    Ok(())
}

fn handle_connect_status(status: &io::Result<std::process::ExitStatus>) {
    match status {
        Ok(s) if s.success() => {
            let host = get_machine_hostname();
            println!(
                "\n  {} Tailscale connected successfully! MagicDNS is active.",
                "✔".green().bold()
            );
            if !host.is_empty() {
                println!(
                    "    Connect via SSH from any linked device: {}",
                    format!("ssh <user>@{host}").cyan().bold()
                );
            }
        }
        _ => {
            println!(
                "\n  {} Tailscale authentication pending.",
                "⚠".yellow().bold()
            );
            println!(
                "    Complete authentication later: {}",
                "sudo tailscale up --ssh".cyan()
            );
        }
    }
}

fn get_machine_hostname() -> String {
    std::process::Command::new("hostname").output().map_or_else(
        |_| String::new(),
        |o| String::from_utf8_lossy(&o.stdout).trim().to_string(),
    )
}
