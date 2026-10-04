//! Headless ALSA null-sink configuration.

use crate::runner::Runner;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

/// Null-sink `~/.asoundrc` that routes the default ALSA PCM to a no-op device.
pub const NULL_ASOUNDRC: &str = "pcm.!default { type null }\nctl.!default { type null }\n";

/// Writes a headless-friendly `~/.asoundrc` so audio-aware tools never block
/// or crash on servers without sound hardware.
///
/// An existing custom config is backed up to `~/.asoundrc.bak` first.
///
/// # Errors
/// Returns an error if the existing config cannot be backed up or written.
pub fn setup(runner: &mut Runner) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let path = Path::new(&home).join(".asoundrc");

    if runner.dry_run {
        println!("  {} [dry-run] write {}", "•".dimmed(), path.display());
        return Ok(());
    }

    if let Some(backup) = backup_if_custom(&path)? {
        println!(
            "  {} Backed up existing ALSA config to {}",
            "•".dimmed(),
            backup.display()
        );
    }

    fs::write(&path, NULL_ASOUNDRC)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    println!(
        "  {} Headless ALSA null sink written to {}",
        "✔".green().bold(),
        path.display()
    );
    Ok(())
}

/// Moves a pre-existing non-null `~/.asoundrc` aside and returns the backup path.
fn backup_if_custom(path: &Path) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let existing = fs::read_to_string(path).unwrap_or_default();
    if existing == NULL_ASOUNDRC {
        return Ok(None);
    }
    let backup = path.with_file_name(".asoundrc.bak");
    fs::rename(path, &backup).with_context(|| format!("Failed to back up {}", path.display()))?;
    Ok(Some(backup))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_asoundrc_routes_both_entries() {
        assert!(NULL_ASOUNDRC.contains("pcm.!default { type null }"));
        assert!(NULL_ASOUNDRC.contains("ctl.!default { type null }"));
    }
}
