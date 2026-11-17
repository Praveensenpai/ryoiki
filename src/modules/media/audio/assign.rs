//! Manually assign an original language to a `[Multi]` file.
//!
//! Remuxes the `[Multi]` sidecar down to one chosen language, removes any
//! wrong-language siblings produced by an earlier (false) selection, and by
//! default deletes the `[Multi]` sidecar so only the clean single-language
//! file remains.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

use super::decisions;
use super::remux::apply_language;
use crate::modules::media::audio::decisions::DecisionRecord;
use crate::modules::media::organizer::find_videos_recursive;
use crate::modules::media::organizer::pathing::{get_jellyfin_media_dir, is_video_file};
use crate::modules::media::probe;

/// Entry point for `ryoiki audio assign`.
pub fn handle_assign(
    path: &str,
    language: &str,
    keep_multi: bool,
    keep_siblings: bool,
    dry_run: bool,
) -> Result<()> {
    let given = PathBuf::from(path);
    let multi_path = find_multi_sidecar(&given)?;
    let multi_path = multi_path.canonicalize().unwrap_or(multi_path);
    let parent = multi_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let multi_name = multi_path
        .file_name()
        .and_then(|n| n.to_str())
        .context("multi file has no name")?
        .to_string();

    let base_key = decisions::decision_key(&multi_name);
    let target_name =
        crate::modules::media::ai::ensure_language_in_clean_name(&multi_name, language);
    let target_path = parent.join(&target_name);

    // Wrong-language siblings sharing the same base title.
    let mut wrong_siblings: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_file() || !is_video_file(&p) {
                continue;
            }
            if p == multi_path || p == target_path {
                continue;
            }
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if decisions::decision_key(name) != base_key {
                continue;
            }
            let Some(lang) = language_tag_of(name) else {
                continue;
            };
            if lang.eq_ignore_ascii_case("multi")
                || lang.eq_ignore_ascii_case("original")
                || lang.eq_ignore_ascii_case(language)
            {
                continue;
            }
            wrong_siblings.push(p);
        }
    }

    println!("  {} Assigning language {}", "🗡️".cyan(), language.bold());
    println!("    Source [Multi]: {}", multi_path.display());
    println!("    Output:         {}", target_path.display());
    if !keep_siblings && !wrong_siblings.is_empty() {
        println!("    Remove wrong-language siblings:");
        for w in &wrong_siblings {
            println!("      - {}", w.display());
        }
    }
    if keep_multi {
        println!("    Delete [Multi] after remux: no (kept)");
    } else {
        println!("    Delete [Multi] after remux: yes");
    }

    if dry_run {
        println!("  {} Dry run — no changes made.", "•".dimmed());
        return Ok(());
    }

    // Remove wrong-language siblings first.
    if !keep_siblings {
        for w in &wrong_siblings {
            if fs::remove_file(w).is_ok() {
                println!("  {} Removed {}", "✔".green(), w.display());
            }
        }
    }

    // Overwrite any stale target.
    if target_path.exists() && target_path != multi_path {
        let _ = fs::remove_file(&target_path);
    }

    let applied = apply_language(&multi_path, language)
        .with_context(|| format!("failed to remux {}", multi_path.display()))?;
    println!("  {} Created {}", "✔".green().bold(), applied.display());

    if !keep_multi && multi_path.exists() {
        let _ = fs::remove_file(&multi_path);
        println!(
            "  {} Deleted [Multi]: {}",
            "✔".green(),
            multi_path.display()
        );
    }

    // Keep the decision store consistent.
    update_decision_store(&base_key, language, &multi_name, &multi_path);

    Ok(())
}

fn find_multi_sidecar(given: &Path) -> Result<PathBuf> {
    if given.is_file() {
        let name = given.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.to_ascii_lowercase().contains("[multi]") {
            return Ok(given.to_path_buf());
        }
        // Look for a sibling [Multi].
        let parent = given
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let key = decisions::decision_key(name);
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = fs::read_dir(parent) {
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_file() {
                    continue;
                }
                let Some(n) = p.file_name().and_then(|s| s.to_str()) else {
                    continue;
                };
                if !n.to_ascii_lowercase().contains("[multi]") {
                    continue;
                }
                if decisions::decision_key(n) != key {
                    continue;
                }
                candidates.push(p);
            }
        }
        return pick_candidate(candidates, parent);
    }

    if given.is_dir() {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = fs::read_dir(given) {
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_file() {
                    continue;
                }
                let Some(n) = p.file_name().and_then(|s| s.to_str()) else {
                    continue;
                };
                if n.to_ascii_lowercase().contains("[multi]") {
                    candidates.push(p);
                }
            }
        }
        return pick_candidate(candidates, given);
    }

    anyhow::bail!("path does not exist: {}", given.display());
}

fn pick_candidate(candidates: Vec<PathBuf>, dir: &Path) -> Result<PathBuf> {
    match candidates.len() {
        0 => anyhow::bail!(
            "no [Multi] sidecar found in {} — nothing to assign from",
            dir.display()
        ),
        1 => Ok(candidates.into_iter().next().context("no candidate")?),
        _ => {
            let list = candidates
                .iter()
                .map(|p| format!("  - {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n");
            anyhow::bail!("multiple [Multi] files found; pass one explicitly:\n{list}");
        }
    }
}

fn language_tag_of(name: &str) -> Option<String> {
    for &known in probe::KNOWN_LANGUAGES {
        let tag = format!("[{known}]");
        if name.contains(&tag) {
            return Some(known.to_string());
        }
    }
    // Also accept arbitrary bracketed single words that look like a language tag
    // positioned where a language would be, but keep it conservative: only known.
    None
}

fn update_decision_store(base_key: &str, language: &str, title: &str, multi_path: &Path) {
    let Ok(mut store) = decisions::load() else {
        return;
    };
    let id = decisions::short_id(base_key);
    let now = decisions::now_secs();

    // Remove any pending entry for this title.
    store.pending.retain(|p| p.id != id);

    if let Some(rec) = store.history.iter_mut().find(|r| r.id == id) {
        rec.chosen = language.to_string();
        rec.decided_secs = now;
        rec.auto = false;
        rec.silenced = false;
        rec.multi_path = Some(multi_path.to_path_buf());
    } else {
        store.history.push(DecisionRecord {
            id,
            title: title.to_string(),
            chosen: language.to_string(),
            decided_secs: now,
            auto: false,
            silenced: false,
            created_secs: now,
            multi_path: Some(multi_path.to_path_buf()),
        });
    }
    let _ = decisions::save(&store);
}

mod interactive;
pub use interactive::interactive_assign;
