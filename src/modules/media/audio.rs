pub mod cli;
pub mod retry_timer;
pub mod strip_queue;

pub use cli::{handle_cli, AudioSubcommand};

use colored::Colorize;
use std::path::{Path, PathBuf};
use std::process::Command;

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

#[derive(Debug)]
pub enum StripOutcome {
    Stripped {
        primary_lang: String,
        original_path: PathBuf,
        multi_path: Option<PathBuf>,
        reclaimed_bytes: u64,
    },
    Preserved,
    Failed,
}

/// Aggregated result of stripping across one organize or retry run.
#[derive(Debug, Default, Clone, Copy)]
pub struct AudioStripSummary {
    pub stripped: usize,
    pub preserved: usize,
    pub failed: usize,
    pub reclaimed_bytes: u64,
}

impl AudioStripSummary {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stripped + self.preserved + self.failed == 0
    }

    pub fn record(&mut self, outcome: &StripOutcome) {
        match outcome {
            StripOutcome::Stripped {
                reclaimed_bytes, ..
            } => {
                self.stripped += 1;
                self.reclaimed_bytes += reclaimed_bytes;
            }
            StripOutcome::Preserved => self.preserved += 1,
            StripOutcome::Failed => self.failed += 1,
        }
    }

    pub fn merge(&mut self, other: &Self) {
        self.stripped += other.stripped;
        self.preserved += other.preserved;
        self.failed += other.failed;
        self.reclaimed_bytes += other.reclaimed_bytes;
    }

    /// One-line HTML summary for the organizer card, or `None` when nothing ran.
    #[must_use]
    pub fn render_html_line(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        let mut parts = vec![format!("{} stripped", self.stripped)];
        if self.preserved > 0 {
            parts.push(format!("{} preserved", self.preserved));
        }
        if self.failed > 0 {
            parts.push(format!("{} failed", self.failed));
        }
        let reclaimed = format_reclaimed(self.reclaimed_bytes);
        Some(format!(
            "🗡️ <b>Audio:</b> {} • reclaimed {reclaimed}",
            parts.join(", ")
        ))
    }
}

fn format_reclaimed(bytes: u64) -> String {
    let mut b = bytes;
    let mut rem = 0;
    let mut unit = "B";
    for u in ["KB", "MB", "GB", "TB"] {
        if b < 1024 {
            break;
        }
        rem = (b % 1024) * 10 / 1024;
        b /= 1024;
        unit = u;
    }
    if unit == "B" {
        format!("{bytes} B")
    } else {
        format!("{b}.{rem} {unit}")
    }
}

impl AudioStripSummary {
    /// Human-readable reclaimed size, for plain-text CLI summaries.
    #[must_use]
    pub fn reclaimed_display(&self) -> String {
        format_reclaimed(self.reclaimed_bytes)
    }
}

/// Automatically strips redundant dub tracks from an organized media file.
///
/// When `preserve_multi` is false (anime), only the original audio track is kept
/// and no `[Multi]` sidecar is written. On failure, enqueues the path for hourly
/// retry (up to 24 attempts).
pub fn strip_audio_auto(path: &Path, preserve_multi: bool) -> StripOutcome {
    let Some(bin) = find_dubstrip_bin() else {
        return StripOutcome::Preserved;
    };

    println!(
        "  {} Verifying native audio tracks via dubstrip...",
        "🗡️".cyan()
    );

    let outcome = strip_and_preserve(&bin, path, preserve_multi);
    match &outcome {
        StripOutcome::Stripped {
            primary_lang,
            original_path,
            multi_path,
            ..
        } => {
            println!(
                "  {} Audio stream optimization complete",
                "✔".green().bold()
            );
            if let Some(orig_name) = original_path.file_name().and_then(|n| n.to_str()) {
                println!("  {} Original [{primary_lang}]: {}", "🎬".cyan(), orig_name);
            }
            if let Some(multi_name) = multi_path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
            {
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
            strip_queue::enqueue(path, preserve_multi);
        }
    }
    outcome
}

/// Attempts to strip unwanted dubs from media.
///
/// When `preserve_multi` is true the full multi-audio version is kept side-by-side
/// as a `[Multi]` file. When false (anime), only the stripped original is kept and
/// no sidecar is written.
pub fn strip_and_preserve(bin: &Path, path: &Path, preserve_multi: bool) -> StripOutcome {
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

    let size_before = std::fs::metadata(path).map_or(0, |m| m.len());

    let temp_multi = parent_dir.join(format!(".{filename}.multi_tmp"));
    if preserve_multi && std::fs::copy(path, &temp_multi).is_err() {
        return StripOutcome::Failed;
    }

    let mut cmd = Command::new(bin);
    cmd.args(["strip", "--auto", "--force", "--quiet"]);
    if !preserve_multi {
        cmd.arg("--anime");
    }
    let succeeded = cmd.arg(path).status().is_ok_and(|s| s.success());

    if !succeeded {
        if preserve_multi {
            let _ = std::fs::remove_file(&temp_multi);
        }
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

        let size_after = std::fs::metadata(path).map_or(size_before, |m| m.len());
        let reclaimed_bytes = size_before.saturating_sub(size_after);

        if preserve_multi {
            if let Some((orig_p, multi_p)) = finalize_dual_versions(path, &temp_multi, primary) {
                StripOutcome::Stripped {
                    primary_lang: primary.to_string(),
                    original_path: orig_p,
                    multi_path: Some(multi_p),
                    reclaimed_bytes,
                }
            } else {
                StripOutcome::Failed
            }
        } else if let Some(orig_p) = finalize_single_version(path, primary) {
            StripOutcome::Stripped {
                primary_lang: primary.to_string(),
                original_path: orig_p,
                multi_path: None,
                reclaimed_bytes,
            }
        } else {
            StripOutcome::Failed
        }
    } else {
        if preserve_multi {
            let _ = std::fs::remove_file(&temp_multi);
        }
        sync_filename_after_strip(path);
        StripOutcome::Preserved
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
mod tests;
