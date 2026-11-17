use super::{
    apply_language, decisions, find_videos_recursive, fs, get_jellyfin_media_dir, is_video_file,
    language_tag_of, probe, update_decision_store, Colorize, Path, PathBuf, Result,
};

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
    let applied = apply_language(multi_path, chosen)?;
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
