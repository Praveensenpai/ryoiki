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

#[derive(Debug)]
pub enum StripOutcome {
    Stripped {
        primary_lang: String,
        original_path: PathBuf,
        multi_path: PathBuf,
    },
    Preserved,
    Failed,
}

/// Automatically strips redundant dub tracks from an organized media file while preserving a [Multi] version next to it.
/// On failure, enqueues the path for hourly retry (up to 24 attempts).
pub fn strip_audio_auto(path: &Path) {
    let Some(bin) = find_dubstrip_bin() else {
        return;
    };

    println!(
        "  {} Verifying native audio tracks via dubstrip...",
        "🗡️".cyan()
    );

    match strip_and_preserve(&bin, path) {
        StripOutcome::Stripped {
            primary_lang,
            original_path,
            multi_path,
        } => {
            println!(
                "  {} Audio stream optimization complete",
                "✔".green().bold()
            );
            if let Some(orig_name) = original_path.file_name().and_then(|n| n.to_str()) {
                println!("  {} Original [{primary_lang}]: {}", "🎬".cyan(), orig_name);
            }
            if let Some(multi_name) = multi_path.file_name().and_then(|n| n.to_str()) {
                println!("  {} Multi Audio: {}", "🎧".cyan(), multi_name);
            }
        }
        StripOutcome::Preserved => {
            println!(
                "  {} Audio preserved (single track or preserved multi)",
                "•".dimmed()
            );
        }
        StripOutcome::Failed => {
            eprintln!("  ⚠️ dubstrip failed — enqueueing for hourly retry");
            strip_queue::enqueue(path);
        }
    }
}

/// Attempts to strip unwanted dubs from media while preserving the full multi-audio version side-by-side.
pub fn strip_and_preserve(bin: &Path, path: &Path) -> StripOutcome {
    let initial_probe = super::probe::probe_media_file(path);
    let orig_stream_count = initial_probe.as_ref().map_or(0, |p| p.audio_stream_count);

    if orig_stream_count <= 1 {
        return StripOutcome::Preserved;
    }

    let Some(parent_dir) = path.parent() else {
        return StripOutcome::Failed;
    };
    let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
        return StripOutcome::Failed;
    };

    let temp_multi = parent_dir.join(format!(".{filename}.multi_tmp"));
    if std::fs::copy(path, &temp_multi).is_err() {
        return StripOutcome::Failed;
    }

    let succeeded = Command::new(bin)
        .args(["strip", "--auto", "--force"])
        .arg(path)
        .status()
        .is_ok_and(|s| s.success());

    if !succeeded {
        let _ = std::fs::remove_file(&temp_multi);
        return StripOutcome::Failed;
    }

    let post_probe = super::probe::probe_media_file(path);
    let post_stream_count = post_probe.as_ref().map_or(0, |p| p.audio_stream_count);

    if post_stream_count < orig_stream_count && post_stream_count > 0 {
        let primary = post_probe
            .as_ref()
            .and_then(|p| p.primary_language.as_deref())
            .filter(|s| !s.is_empty())
            .unwrap_or("Original");

        if let Some((orig_p, multi_p)) = finalize_dual_versions(path, &temp_multi, primary) {
            StripOutcome::Stripped {
                primary_lang: primary.to_string(),
                original_path: orig_p,
                multi_path: multi_p,
            }
        } else {
            StripOutcome::Failed
        }
    } else {
        let _ = std::fs::remove_file(&temp_multi);
        sync_filename_after_strip(path);
        StripOutcome::Preserved
    }
}

fn finalize_dual_versions(
    path: &Path,
    temp_multi: &Path,
    primary: &str,
) -> Option<(PathBuf, PathBuf)> {
    let parent_dir = path.parent()?;
    let orig_filename = path.file_name()?.to_str()?;

    let stripped_name =
        crate::modules::media::ai::ensure_language_in_clean_name(orig_filename, primary);
    let stripped_path = parent_dir.join(&stripped_name);

    let multi_name =
        crate::modules::media::ai::ensure_language_in_clean_name(orig_filename, "Multi");
    let multi_path = parent_dir.join(&multi_name);

    if path != stripped_path {
        if stripped_path.exists() {
            let _ = std::fs::remove_file(&stripped_path);
        }
        let _ = std::fs::rename(path, &stripped_path);
    }

    if multi_path.exists() && multi_path != temp_multi {
        let _ = std::fs::remove_file(&multi_path);
    }

    if std::fs::rename(temp_multi, &multi_path).is_ok() {
        Some((stripped_path, multi_path))
    } else {
        let _ = std::fs::remove_file(temp_multi);
        None
    }
}

/// Synchronizes the file's language tag with the probed native audio language after stripping.
pub fn sync_filename_after_strip(path: &Path) {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if filename.is_empty() {
        return;
    }

    let Some(probe) = super::probe::probe_media_file(path) else {
        return;
    };

    let Some(ref primary) = probe.primary_language else {
        return;
    };

    if primary.is_empty() {
        return;
    }

    let new_filename = crate::modules::media::ai::ensure_language_in_clean_name(filename, primary);
    if new_filename != filename {
        let new_path = path.with_file_name(&new_filename);
        if let Ok(()) = std::fs::rename(path, &new_path) {
            println!(
                "  {} Updated media tag: {} -> [{primary}]",
                "✔".green().bold(),
                filename
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_find_dubstrip_bin() {
        let _ = find_dubstrip_bin();
    }

    #[test]
    fn test_finalize_dual_versions_creates_both_files() -> Result<()> {
        let tmp = std::env::temp_dir().join(format!("ryoiki_test_dual_{}", std::process::id()));
        fs::create_dir_all(&tmp)?;

        let path = tmp.join("Sample Movie (2024) [Multi] [1080p].mkv");
        let temp_multi = tmp.join(".Sample Movie (2024) [Multi] [1080p].mkv.multi_tmp");

        fs::write(&path, b"mock stripped native audio")?;
        fs::write(&temp_multi, b"mock multi audio")?;

        let res = finalize_dual_versions(&path, &temp_multi, "Japanese");
        let (orig_p, multi_p) = res.context("Expected dual versions to be created")?;

        assert_eq!(
            orig_p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default(),
            "Sample Movie (2024) [Japanese] [1080p].mkv"
        );
        assert_eq!(
            multi_p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default(),
            "Sample Movie (2024) [Multi] [1080p].mkv"
        );

        assert!(orig_p.exists());
        assert!(multi_p.exists());
        assert_eq!(fs::read_to_string(&orig_p)?, "mock stripped native audio");
        assert_eq!(fs::read_to_string(&multi_p)?, "mock multi audio");

        let _ = fs::remove_dir_all(&tmp);
        Ok(())
    }
}
