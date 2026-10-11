use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;
use std::process::Command;

use super::{find_dubstrip_bin, strip_queue};

#[derive(Subcommand, Debug, Clone)]
pub enum AudioSubcommand {
    /// Inspect audio streams, detected origin, and planned actions
    Inspect {
        /// Path to video file
        path: String,
    },
    /// Strip unwanted dub audio from a video file
    Strip {
        /// Path to video file
        path: String,
        /// Run non-interactively without confirmation prompt
        #[arg(long)]
        auto: bool,
        /// Preview planned track removals without altering file
        #[arg(long)]
        dry_run: bool,
        /// Skip 4-gate download safety checks
        #[arg(long)]
        force: bool,
    },
    /// Sweep a directory and strip unwanted dub audio across all media
    Sweep {
        /// Path to media folder
        path: String,
        /// Run non-interactively without confirmation prompt
        #[arg(long)]
        auto: bool,
        /// Preview planned track removals without altering files
        #[arg(long)]
        dry_run: bool,
        /// Skip 4-gate download safety checks
        #[arg(long)]
        force: bool,
    },
    /// Process the pending strip retry queue (runs hourly via systemd)
    Retry,
    /// List in-progress language decisions, or send daily reminders (`--notify`)
    Pending {
        /// Send Telegram prompts and apply overdue decisions
        #[arg(long)]
        notify: bool,
    },
}

/// Dispatches CLI subcommands to the standalone `dubstrip` engine.
pub fn handle_cli(sub: AudioSubcommand) -> Result<()> {
    match sub {
        AudioSubcommand::Retry => return handle_retry(),
        AudioSubcommand::Pending { notify } => return handle_pending(notify),
        AudioSubcommand::Inspect { .. }
        | AudioSubcommand::Strip { .. }
        | AudioSubcommand::Sweep { .. } => {}
    }

    let Some(bin) = find_dubstrip_bin() else {
        println!(
            "\n  {} dubstrip utility not found in PATH or ~/.local/bin/dubstrip",
            "✖".red().bold()
        );
        println!("    Install with: cargo install --path /home/neko/dubstrip\n");
        return Ok(());
    };

    let mut cmd = Command::new(bin);
    match sub {
        AudioSubcommand::Inspect { path } => {
            cmd.arg("inspect").arg(path);
        }
        AudioSubcommand::Strip {
            path,
            auto,
            dry_run,
            force,
        } => {
            cmd.arg("strip");
            append_flags(&mut cmd, auto, dry_run, force);
            cmd.arg(path);
        }
        AudioSubcommand::Sweep {
            path,
            auto,
            dry_run,
            force,
        } => {
            cmd.arg("sweep");
            append_flags(&mut cmd, auto, dry_run, force);
            cmd.arg(path);
        }
        AudioSubcommand::Retry | AudioSubcommand::Pending { .. } => unreachable!(),
    }

    let status = cmd.status().context("Failed to invoke dubstrip process")?;
    if !status.success() {
        anyhow::bail!("dubstrip exited with status: {status}");
    }
    Ok(())
}

fn handle_pending(notify: bool) -> Result<()> {
    if notify {
        let config = crate::notify::config::TelegramConfig::load()
            .context("Failed to load Telegram config for pending reminders")?;
        let sent = super::approval::notify_pending(&config)?;
        println!(
            "  {} {sent} pending reminder(s) dispatched",
            "✔".green().bold()
        );
    }
    println!("{}", super::approval::list_pending_text());
    Ok(())
}

fn handle_retry() -> Result<()> {
    let Some(bin) = find_dubstrip_bin() else {
        println!(
            "  {} dubstrip not found — skipping retry queue",
            "•".dimmed()
        );
        return Ok(());
    };
    strip_queue::process_queue(&bin).map(|_| ())
}

fn append_flags(cmd: &mut Command, auto: bool, dry_run: bool, force: bool) {
    if auto {
        cmd.arg("--auto");
    }
    if dry_run {
        cmd.arg("--dry-run");
    }
    if force {
        cmd.arg("--force");
    }
}
