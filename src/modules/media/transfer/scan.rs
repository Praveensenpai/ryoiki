use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process::Command;

use super::{MediaCategory, MediaItem, MediaSeason};

pub fn calculate_dir_size(path: &Path) -> u64 {
    if path.is_file() {
        return fs::metadata(path).map_or(0, |m| m.len());
    }
    let mut total: u64 = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                total = total.saturating_add(fs::metadata(&p).map_or(0, |m| m.len()));
            } else if p.is_dir() {
                total = total.saturating_add(calculate_dir_size(&p));
            }
        }
    }
    total
}

pub fn scan_local_media(home: &Path) -> Vec<MediaItem> {
    let mut items = Vec::new();
    let media_base = home.join("jellyfin/media");

    scan_local_category(&media_base, "movies", MediaCategory::Movie, &mut items);
    scan_local_category(&media_base, "shows", MediaCategory::Show, &mut items);
    scan_local_category(&media_base, "anime", MediaCategory::Anime, &mut items);

    tag_watched_local(&mut items);
    items.sort_by_key(|a| a.title.to_lowercase());
    items
}

fn scan_local_category(base: &Path, folder: &str, cat: MediaCategory, items: &mut Vec<MediaItem>) {
    let cat_dir = base.join(folder);
    let Ok(entries) = fs::read_dir(&cat_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }

        let remote_rel = format!("media/{}/{}", cat.remote_folder(), name);
        let seasons = if cat == MediaCategory::Movie {
            Vec::new()
        } else {
            detect_seasons(&path, &remote_rel, true)
        };

        let size = if seasons.is_empty() {
            calculate_dir_size(&path)
        } else {
            seasons.iter().map(|s| s.size_bytes).sum()
        };

        items.push(MediaItem {
            title: name,
            category: cat,
            size_bytes: size,
            is_watched: false,
            local_path: Some(path),
            remote_path: remote_rel,
            seasons,
        });
    }
}

pub fn scan_remote_media(home: &Path) -> Result<Vec<MediaItem>> {
    let mut items = Vec::new();
    let gdrive_media = home.join("gdrive/media");

    if gdrive_media.exists() {
        scan_remote_mounted(&gdrive_media, "movie", MediaCategory::Movie, &mut items);
        scan_remote_mounted(&gdrive_media, "anime", MediaCategory::Anime, &mut items);
        scan_remote_mounted(
            &gdrive_media,
            "anime/movie",
            MediaCategory::Anime,
            &mut items,
        );
        scan_remote_mounted(&gdrive_media, "shows", MediaCategory::Show, &mut items);
    } else {
        scan_remote_via_rclone("media/movie", MediaCategory::Movie, &mut items)?;
        scan_remote_via_rclone("media/anime", MediaCategory::Anime, &mut items)?;
        scan_remote_via_rclone("media/anime/movie", MediaCategory::Anime, &mut items)?;
    }

    items.sort_by_key(|a| a.title.to_lowercase());
    Ok(items)
}

fn scan_remote_mounted(base: &Path, folder: &str, cat: MediaCategory, items: &mut Vec<MediaItem>) {
    let folder_path = base.join(folder);
    let Ok(entries) = fs::read_dir(folder_path) else {
        return;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || (folder == "anime" && name == "movie") {
            continue;
        }

        let remote_path = format!("media/{folder}/{name}");
        let seasons = if cat == MediaCategory::Movie {
            Vec::new()
        } else {
            detect_seasons(&entry.path(), &remote_path, false)
        };

        let size = if seasons.is_empty() {
            calculate_dir_size(&entry.path())
        } else {
            seasons.iter().map(|s| s.size_bytes).sum()
        };

        items.push(MediaItem {
            title: name,
            category: cat,
            size_bytes: size,
            is_watched: false,
            local_path: None,
            remote_path,
            seasons,
        });
    }
}

fn detect_seasons(dir_path: &Path, remote_base: &str, is_local: bool) -> Vec<MediaSeason> {
    let mut seasons = Vec::new();
    let Ok(entries) = fs::read_dir(dir_path) else {
        return seasons;
    };

    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with('.') {
                subdirs.push((name, p));
            }
        }
    }

    subdirs.sort_by_key(|(name, _)| name.to_lowercase());

    for (name, path) in subdirs {
        let size = calculate_dir_size(&path);
        let remote_path = format!("{remote_base}/{name}");
        seasons.push(MediaSeason {
            title: name,
            size_bytes: size,
            local_path: if is_local { Some(path) } else { None },
            remote_path,
            is_selected: false,
        });
    }

    seasons
}

fn scan_remote_via_rclone(
    remote_dir: &str,
    cat: MediaCategory,
    items: &mut Vec<MediaItem>,
) -> Result<()> {
    let out = Command::new("rclone")
        .args(["lsf", "--dirs-only", &format!("gdrive:{remote_dir}")])
        .output()?;

    if !out.status.success() {
        return Ok(());
    }

    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let name = line.trim().trim_end_matches('/').to_string();
        if name.is_empty() || (remote_dir == "media/anime" && name == "movie") {
            continue;
        }
        let remote_path = format!("{remote_dir}/{name}");
        items.push(MediaItem {
            title: name,
            category: cat,
            size_bytes: 0,
            is_watched: false,
            local_path: None,
            remote_path,
            seasons: Vec::new(),
        });
    }
    Ok(())
}

fn tag_watched_local(items: &mut [MediaItem]) {
    let url = "http://localhost:8096/Items?Filters=IsPlayed&Recursive=true&Fields=Path";
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    else {
        return;
    };

    let Ok(resp) = client.get(url).send() else {
        return;
    };
    let Ok(json) = resp.json::<serde_json::Value>() else {
        return;
    };

    let Some(played) = json.get("Items").and_then(|i| i.as_array()) else {
        return;
    };

    for item in played {
        if let Some(p) = item.get("Path").and_then(|p| p.as_str()) {
            for it in items.iter_mut() {
                if it
                    .local_path
                    .as_ref()
                    .is_some_and(|lp| p.contains(&*lp.to_string_lossy()))
                {
                    it.is_watched = true;
                }
            }
        }
    }
}
