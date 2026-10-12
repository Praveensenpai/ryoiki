//! Dub-stripping pipeline: runs `dubstrip`, finalizes the dual/single versions,
//! and stages the original-language decision when a multi-track file survives.

use colored::Colorize;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::super::probe::{self, MediaProbe};
use super::{approval, decisions, StripOutcome};

/// Attempts to strip unwanted dubs from media.
///
/// When `preserve_multi` is true the full multi-audio version is kept side-by-side
/// as a `[Multi]` file. When false (anime), only the stripped original is kept and
/// no sidecar is written.
pub fn strip_and_preserve(bin: &Path, path: &Path, preserve_multi: bool) -> StripOutcome {
    let orig_stream_count = probe(path).as_ref().map_or(0, |p| p.audio_stream_count);
    if orig_stream_count <= 1 {
        return StripOutcome::Preserved;
    }

    let Some(parent_dir) = path.parent() else {
        return StripOutcome::Failed;
    };
    let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
        return StripOutcome::Failed;
    };

    let size_before = std::fs::metadata(path).map_or(0, |m| m.len());
    let temp_multi = parent_dir.join(format!(".{filename}.multi_tmp"));
    if preserve_multi && std::fs::copy(path, &temp_multi).is_err() {
        return StripOutcome::Failed;
    }

    if !run_dubstrip(bin, path, preserve_multi) {
        cleanup_temp(&temp_multi, preserve_multi);
        return StripOutcome::Failed;
    }

    let post_probe = probe(path);
    let post_stream_count = post_probe.as_ref().map_or(0, |p| p.audio_stream_count);
    if post_stream_count < orig_stream_count && post_stream_count > 0 {
        finalize_stripped(
            path,
            &temp_multi,
            post_probe.as_ref(),
            size_before,
            preserve_multi,
        )
    } else {
        cleanup_temp(&temp_multi, preserve_multi);
        preserved(path, preserve_multi)
    }
}

/// Runs dubstrip once. Returns `false` when it failed or declined via a safety gate.
fn run_dubstrip(bin: &Path, path: &Path, preserve_multi: bool) -> bool {
    let mut cmd = Command::new(bin);
    cmd.args(["strip", "--auto", "--force", "--quiet"]);
    if !preserve_multi {
        cmd.arg("--anime");
    }
    let Ok(out) = cmd.arg(path).output() else {
        return false;
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // A safety-gate skip (open handle, freshly moved, incomplete download) is
    // transient: report failure so the caller retries instead of preserving.
    if stdout.contains("Skipping") || stderr.contains("Skipping") {
        return false;
    }
    out.status.success()
}

fn cleanup_temp(temp_multi: &Path, preserve_multi: bool) {
    if preserve_multi {
        let _ = std::fs::remove_file(temp_multi);
    }
}

/// Produces the `Stripped` outcome after a successful reduction.
fn finalize_stripped(
    path: &Path,
    temp_multi: &Path,
    post_probe: Option<&MediaProbe>,
    size_before: u64,
    preserve_multi: bool,
) -> StripOutcome {
    let primary = post_probe
        .and_then(|p| p.primary_language.as_deref())
        .filter(|s| !s.is_empty())
        .unwrap_or("Original");
    let size_after = std::fs::metadata(path).map_or(size_before, |m| m.len());
    let reclaimed_bytes = size_before.saturating_sub(size_after);

    let (original_path, multi_path) = if preserve_multi {
        match finalize_dual_versions(path, temp_multi, primary) {
            Some((orig, multi)) => (orig, Some(multi)),
            None => return StripOutcome::Failed,
        }
    } else {
        match finalize_single_version(path, primary) {
            Some(orig) => (orig, None),
            None => return StripOutcome::Failed,
        }
    };

    StripOutcome::Stripped {
        primary_lang: primary.to_string(),
        original_path,
        multi_path,
        reclaimed_bytes,
    }
}

/// Handles a file dubstrip declined to reduce.
fn preserved(path: &Path, preserve_multi: bool) -> StripOutcome {
    // Only retag when the file is truly single-track. A still-multi-track file
    // keeps its `[Multi]` marker and is staged for a language choice.
    if !stage_preserved_if_multi(path, preserve_multi) {
        sync_filename_after_strip(path);
    }
    StripOutcome::Preserved
}

fn probe(path: &Path) -> Option<MediaProbe> {
    probe::probe_media_file(path)
}

/// Stages a freshly stripped multi-audio file for an original-language decision.
///
/// The AI recommendation is attached when available, and repeat titles are
/// reported as duplicates instead of prompted again.
pub fn stage_language_decision(stripped_path: &Path, multi_path: &Path) {
    let probe = probe(multi_path);
    let name = multi_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let recommended = approval::recommend(name, probe.as_ref());
    match approval::register(
        stripped_path,
        multi_path,
        probe.as_ref(),
        recommended.as_deref(),
    ) {
        approval::StageOutcome::Pending => {
            println!(
                "  {} Original language pending — reply in Telegram within {} days",
                "❓".cyan(),
                decisions::PENDING_DAYS
            );
            if let Ok(config) = crate::notify::config::TelegramConfig::load() {
                let _ = approval::notify_pending(&config);
            }
        }
        approval::StageOutcome::Duplicate(reason) => {
            println!("  {} {reason}", "•".dimmed());
        }
        approval::StageOutcome::Failed => {}
    }
}

/// Renames the stripped file to carry the primary language tag, without a Multi sidecar.
fn finalize_single_version(path: &Path, primary: &str) -> Option<PathBuf> {
    let parent_dir = path.parent()?;
    let orig_filename = path.file_name()?.to_str()?;
    let stripped_name =
        crate::modules::media::ai::ensure_language_in_clean_name(orig_filename, primary);
    let stripped_path = parent_dir.join(&stripped_name);

    if path != stripped_path {
        if stripped_path.exists() {
            let _ = std::fs::remove_file(&stripped_path);
        }
        if std::fs::rename(path, &stripped_path).is_err() {
            return None;
        }
    }
    Some(stripped_path)
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
fn sync_filename_after_strip(path: &Path) {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if filename.is_empty() {
        return;
    }
    let Some(probe) = probe(path) else {
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

/// Stages a language decision when dubstrip preserved a still-multi-track file.
///
/// Only applies to the non-anime path: there the full multi version is the
/// source of truth and stays on disk while a single-language copy is produced.
///
/// Returns `true` when a decision was staged, so the caller leaves the file's
/// `[Multi]` name untouched.
fn stage_preserved_if_multi(path: &Path, preserve_multi: bool) -> bool {
    if !preserve_multi {
        return false;
    }
    if probe(path).as_ref().map_or(0, |p| p.audio_stream_count) > 1 {
        stage_language_decision(path, path);
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Context, Result};
    use std::fs;

    #[test]
    fn test_finalize_dual_versions_creates_both_files() -> Result<()> {
        let tmp = std::env::temp_dir().join(format!("ryoiki_test_dual_{}", std::process::id()));
        fs::create_dir_all(&tmp)?;

        let path = tmp.join("Sample Movie (2024) [Multi] [1080p].mkv");
        let temp_multi = tmp.join(".Sample Movie (2024) [Multi] [1080p].mkv.multi_tmp");
        fs::write(&path, b"mock stripped native audio")?;
        fs::write(&temp_multi, b"mock multi audio")?;

        let (orig_p, multi_p) =
            finalize_dual_versions(&path, &temp_multi, "Japanese").context("dual versions")?;

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
        assert_eq!(fs::read_to_string(&orig_p)?, "mock stripped native audio");
        assert_eq!(fs::read_to_string(&multi_p)?, "mock multi audio");

        let _ = fs::remove_dir_all(&tmp);
        Ok(())
    }

    #[test]
    fn test_finalize_single_version_has_no_multi() -> Result<()> {
        let tmp = std::env::temp_dir().join(format!("ryoiki_test_single_{}", std::process::id()));
        fs::create_dir_all(&tmp)?;

        let path = tmp.join("Anime Show (2024) [Multi] [1080p].mkv");
        fs::write(&path, b"mock japanese only")?;

        let orig = finalize_single_version(&path, "Japanese").context("single version")?;
        assert_eq!(
            orig.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default(),
            "Anime Show (2024) [Japanese] [1080p].mkv"
        );
        assert!(orig.exists());
        assert!(!tmp.join("Anime Show (2024) [Multi] [1080p].mkv").exists());

        let _ = fs::remove_dir_all(&tmp);
        Ok(())
    }
}
