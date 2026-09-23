pub mod retry_timer;
pub mod strip_queue;

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::process::Command;

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
}

/// Locates the installed `dubstrip` binary in PATH or user's local bin.
#[must_use]
pub fn find_dubstrip_bin() -> Option<PathBuf> {
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("dubstrip");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let home = std::env::var("HOME").ok()?;
    let fallback = PathBuf::from(home).join(".local/bin/dubstrip");
    if fallback.is_file() {
        Some(fallback)
    } else {
        None
    }
}

/// Dispatches CLI subcommands to the standalone `dubstrip` engine.
pub fn handle_cli(sub: AudioSubcommand) -> Result<()> {
    match sub {
        AudioSubcommand::Retry => return handle_retry(),
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
        AudioSubcommand::Retry => unreachable!(),
    }

    let status = cmd.status().context("Failed to invoke dubstrip process")?;
    if !status.success() {
        anyhow::bail!("dubstrip exited with status: {status}");
    }
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
    strip_queue::process_queue(&bin)
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

/// Automatically strips redundant dub tracks from an organized media file.
/// On failure, enqueues the path for hourly retry (up to 24 attempts).
pub fn strip_audio_auto(path: &Path) {
    let Some(bin) = find_dubstrip_bin() else {
        return;
    };

    println!(
        "  {} Verifying native audio tracks via dubstrip...",
        "🗡️".cyan()
    );

    let succeeded = Command::new(&bin)
        .args(["strip", "--auto", "--force"])
        .arg(path)
        .status()
        .is_ok_and(|s| s.success());

    if succeeded {
        println!(
            "  {} Audio stream optimization complete",
            "✔".green().bold()
        );
    } else {
        eprintln!("  ⚠️ dubstrip failed — enqueueing for hourly retry");
        strip_queue::enqueue(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_dubstrip_bin() {
        let _ = find_dubstrip_bin();
    }
}
