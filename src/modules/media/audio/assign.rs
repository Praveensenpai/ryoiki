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

/// Interactive picker: list `[Multi]` files, let the user choose one and a
/// language (or "none" to delete all multi audio), then execute.
pub fn interactive_assign() -> Result<()> {
    use std::io::{self, Write};

    let root = get_jellyfin_media_dir();
    let multis = collect_multi_files(&root);
    if multis.is_empty() {
        println!(
            "  {} No [Multi] files found under {}",
            "•".dimmed(),
            root.display()
        );
        return Ok(());
    }

    println!();
    println!("  {} Multi-audio files:", "🎬".cyan().bold());
    for (i, p) in multis.iter().enumerate() {
        println!("    {}. {}", i + 1, p.display());
    }
    println!("    0. Cancel");
    print!("  Choose a file: ");
    io::stdout().flush()?;
    let choice = read_usize();
    if choice == 0 || choice > multis.len() {
        println!("  {} Cancelled.", "•".dimmed());
        return Ok(());
    }
    let multi_path = multis[choice - 1]
        .clone()
        .canonicalize()
        .unwrap_or_else(|_| multis[choice - 1].clone());

    let multi_name = multi_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    let base_key = decisions::decision_key(&multi_name);
    let parent = multi_path.parent().unwrap_or_else(|| Path::new("."));

    let langs = collect_language_options(&multi_path, parent, &base_key);

    println!();
    println!("  {} Available audio languages:", "🎚".cyan().bold());
    for (i, l) in langs.iter().enumerate() {
        println!("    {}. {}", i + 1, l);
    }
    let none_idx = langs.len() + 1;
    println!("    {none_idx}. None (delete all multi audio)");
    println!("    0. Cancel");
    print!("  Choose: ");
    io::stdout().flush()?;
    let choice = read_usize();
    if choice == 0 {
        println!("  {} Cancelled.", "•".dimmed());
        return Ok(());
    }

    if choice == none_idx {
        delete_all_multi(&multi_path, parent, &base_key);
        return Ok(());
    }
    if choice > langs.len() {
        println!("  {} Invalid choice.", "✖".red().bold());
        return Ok(());
    }
    let chosen = langs[choice - 1].clone();
    assign_chosen(&multi_path, &chosen, parent, &base_key, &multi_name)
}

fn collect_multi_files(root: &Path) -> Vec<PathBuf> {
    let mut videos = Vec::new();
    if root.is_dir() {
        let _ = find_videos_recursive(root, &mut videos);
    } else if is_video_file(root) {
        videos.push(root.to_path_buf());
    }
    let mut multis: Vec<PathBuf> = videos
        .into_iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.to_ascii_lowercase().contains("[multi]"))
        })
        .collect();
    multis.sort();
    multis
}

fn collect_language_options(multi_path: &Path, parent: &Path, base_key: &str) -> Vec<String> {
    let mut langs: Vec<String> = Vec::new();
    if let Some(probe) = probe::probe_media_file(multi_path) {
        for l in &probe.audio_languages {
            if !langs.iter().any(|x| x.eq_ignore_ascii_case(l)) {
                langs.push(l.clone());
            }
        }
    }
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_file() || !is_video_file(&p) {
                continue;
            }
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if decisions::decision_key(name) != base_key {
                continue;
            }
            if name.to_ascii_lowercase().contains("[multi]") {
                continue;
            }
            if let Some(lang) = language_tag_of(name) {
                if !lang.eq_ignore_ascii_case("multi")
                    && !lang.eq_ignore_ascii_case("original")
                    && !langs.iter().any(|x| x.eq_ignore_ascii_case(&lang))
                {
                    langs.push(lang);
                }
            }
        }
    }
    langs
}

fn read_usize() -> usize {
    use std::io::{self, BufRead};
    let mut buf = String::new();
    io::stdin().lock().read_line(&mut buf).ok();
    buf.trim().parse().unwrap_or(0)
}

fn delete_all_multi(multi_path: &Path, parent: &Path, base_key: &str) {
    println!();
    let mut removed = 0usize;
    if multi_path.exists() {
        let _ = fs::remove_file(multi_path);
        println!("  {} Deleted {}", "✔".green(), multi_path.display());
        removed += 1;
    }
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_file() || !is_video_file(&p) {
                continue;
            }
            let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if decisions::decision_key(name) != *base_key {
                continue;
            }
            if language_tag_of(name).is_none() {
                continue;
            }
            if fs::remove_file(&p).is_ok() {
                println!("  {} Deleted {}", "✔".green(), p.display());
                removed += 1;
            }
        }
    }
    println!(
        "  {} Removed {} multi-audio file(s).",
        "✔".green().bold(),
        removed
    );
}

fn assign_chosen(
    multi_path: &Path,
    chosen: &str,
    parent: &Path,
    base_key: &str,
    multi_name: &str,
) -> Result<()> {
    let target_name = crate::modules::media::ai::ensure_language_in_clean_name(multi_name, chosen);
    let target_path = parent.join(&target_name);
    let mut wrong: Vec<PathBuf> = Vec::new();
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
            if decisions::decision_key(name) != *base_key {
                continue;
            }
            if let Some(lang) = language_tag_of(name) {
                if !lang.eq_ignore_ascii_case("multi")
                    && !lang.eq_ignore_ascii_case("original")
                    && !lang.eq_ignore_ascii_case(chosen)
                {
                    wrong.push(p);
                }
            }
        }
    }
    for w in &wrong {
        let _ = fs::remove_file(w);
        println!("  {} Removed {}", "✔".green(), w.display());
    }
    if target_path.exists() && target_path != multi_path {
        let _ = fs::remove_file(&target_path);
    }
    let applied = super::remux::apply_language(multi_path, chosen)?;
    println!("  {} Created {}", "✔".green().bold(), applied.display());
    if multi_path.exists() {
        let _ = fs::remove_file(multi_path);
        println!(
            "  {} Deleted [Multi]: {}",
            "✔".green(),
            multi_path.display()
        );
    }
    update_decision_store(base_key, chosen, multi_name, multi_path);
    Ok(())
}
