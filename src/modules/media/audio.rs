pub mod approval;
pub mod cli;
pub mod decisions;
pub mod pending_timer;
pub mod remux;
pub mod retry_timer;
pub mod strip;
pub mod strip_queue;

pub use cli::{handle_cli, AudioSubcommand};
use colored::Colorize;
use std::path::{Path, PathBuf};

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

    /// Human-readable reclaimed size, for plain-text CLI summaries.
    #[must_use]
    pub fn reclaimed_display(&self) -> String {
        format_reclaimed(self.reclaimed_bytes)
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

    let outcome = strip::strip_and_preserve(&bin, path, preserve_multi);
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
            if let Some(multi) = multi_path.as_deref() {
                strip::stage_language_decision(original_path, multi);
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

#[cfg(test)]
mod tests;
