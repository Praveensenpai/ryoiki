mod install;
mod oauth;
mod service;

use crate::runner::Runner;
use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

pub use oauth::is_remote_configured;

/// CLI subcommands for Rclone Google Drive management.
#[derive(Subcommand, Debug, Clone, Copy)]
pub enum RcloneSubcommand {
    /// Run the interactive Google Drive setup wizard
    Setup,
    /// Inspect Google Drive mount and systemd service status
    Status,
    /// Start and mount Google Drive (~/gdrive)
    Mount,
    /// Unmount Google Drive and stop the background service
    Unmount,
}

/// Sets up Rclone and provisions persistent Google Drive systemd user mount.
pub fn setup(runner: &mut Runner, non_interactive: bool) -> Result<()> {
    install::install_rclone_binary(runner)?;
    install::install_fuse_support(runner)?;
    install::configure_fuse_allow_other(runner)?;

    if runner.dry_run {
        println!(
            "  {} [dry-run] Google Drive systemd mount setup",
            "•".dimmed()
        );
        return Ok(());
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let mount_dir = Path::new(&home).join("gdrive");
    fs::create_dir_all(&mount_dir)
        .with_context(|| format!("Failed to create mount directory: {}", mount_dir.display()))?;

    if oauth::is_remote_configured() {
        println!(
            "  {} Google Drive remote 'gdrive:' is authenticated",
            "✔".green()
        );
    } else if non_interactive {
        show_non_interactive_notice();
        return Ok(());
    } else {
        let _ = oauth::prompt_and_configure_remote();
    }

    if !oauth::is_remote_configured() {
        println!(
            "  {} Authentication skipped or token not saved. Mount service was not started.",
            "⚠".yellow().bold()
        );
        println!(
            "    Run {} when ready to complete linking.",
            "ryoiki rclone setup".cyan().bold()
        );
        return Ok(());
    }

    service::deploy_systemd_service(runner, &home)?;
    service::enable_and_start_service(runner)?;
    show_mount_info(&home);
    Ok(())
}

/// Dispatches CLI subcommands for rclone operations.
pub fn handle_cli(sub: RcloneSubcommand) -> Result<()> {
    match sub {
        RcloneSubcommand::Setup => {
            let mut runner = Runner::new(false, false)?;
            setup(&mut runner, false)?;
        }
        RcloneSubcommand::Status => show_status(),
        RcloneSubcommand::Mount => service::start_mount()?,
        RcloneSubcommand::Unmount => service::stop_mount()?,
    }
    Ok(())
}

fn show_non_interactive_notice() {
    println!(
        "  {} Remote 'gdrive:' not yet configured in rclone.",
        "⚠".yellow().bold()
    );
    println!(
        "    Run {} to link your Google account.",
        "ryoiki rclone setup".cyan().bold()
    );
}

fn show_mount_info(home: &str) {
    println!(
        "  {} Google Drive mount target: {}/gdrive",
        "✔".green().bold(),
        home
    );
    println!(
        "    Check status anytime: {}",
        "ryoiki rclone status".cyan().bold()
    );
}

fn show_status() {
    println!(
        "\n  {} {}",
        "☁️".cyan(),
        "Rclone Google Drive Status".bold()
    );
    println!("  {}\n", "─".repeat(40).dimmed());

    let rclone_ok = Runner::command_exists("rclone");
    print_status_item(
        "Rclone Binary",
        if rclone_ok {
            "✔ installed"
        } else {
            "✖ missing"
        },
        rclone_ok,
    );

    let remote_ok = oauth::is_remote_configured();
    print_status_item(
        "Remote 'gdrive:'",
        if remote_ok {
            "✔ configured"
        } else {
            "✖ unconfigured"
        },
        remote_ok,
    );

    let service_active = service::is_service_active();
    print_status_item(
        "Systemd Service",
        if service_active {
            "✔ active (running)"
        } else {
            "✖ inactive"
        },
        service_active,
    );

    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let mount_point = PathBuf::from(&home).join("gdrive");
    let is_mounted = service::is_path_mounted(&mount_point);
    print_status_item(
        "Mount (~/gdrive)",
        if is_mounted {
            "✔ mounted"
        } else {
            "✖ not mounted"
        },
        is_mounted,
    );

    if remote_ok && is_mounted {
        service::print_quota_stats();
    }
    println!();
}

fn print_status_item(label: &str, status: &str, ok: bool) {
    let colored_status = if ok {
        status.green().bold()
    } else {
        status.red().dimmed()
    };
    println!("    {label:<18} {colored_status}");
}
