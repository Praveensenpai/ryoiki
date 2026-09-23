use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use super::cache::MediaScanCache;
use super::{MediaCategory, MediaFile, MediaItem, SyncStatus};

pub fn collect_dir_files(path: &Path, rel_prefix: &str) -> Vec<MediaFile> {
    let mut files = Vec::new();
    if path.is_file() {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().to_string());
        let size = fs::metadata(path).map_or(0, |m| m.len());
        files.push(MediaFile {
            name: name.clone(),
            rel_path: if rel_prefix.is_empty() {
                name
            } else {
                format!("{rel_prefix}/{name}")
            },
            size_bytes: size,
            exists_in_other: false,
        });
        return files;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return files;
    };

    for entry in entries.flatten() {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let sub_rel = if rel_prefix.is_empty() {
            name.clone()
        } else {
            format!("{rel_prefix}/{name}")
        };
        if p.is_file() {
            let size = fs::metadata(&p).map_or(0, |m| m.len());
            files.push(MediaFile {
                name,
                rel_path: sub_rel,
                size_bytes: size,
                exists_in_other: false,
            });
        } else if p.is_dir() {
            files.extend(collect_dir_files(&p, &sub_rel));
        }
    }
    files.sort_by_key(|a| a.name.to_lowercase());
    files
}

pub fn scan_libraries(home: &Path) -> Result<(Vec<MediaItem>, Vec<MediaItem>)> {
    let mut cache = super::cache::load_cache(home);
    let mut local = scan_local_media(home, &mut cache);
    let mut remote = scan_remote_media(home, &mut cache)?;
    super::cross_reference_libraries(&mut local, &mut remote);
    let _ = cache.save(home);
    Ok((local, remote))
}

pub fn rescan_libraries(home: &Path) -> Result<(Vec<MediaItem>, Vec<MediaItem>)> {
    let _ = MediaScanCache::invalidate(home);
    scan_libraries(home)
}

pub fn scan_local_media(home: &Path, cache: &mut MediaScanCache) -> Vec<MediaItem> {
    let mut items = Vec::new();
    let mut found_keys = HashSet::new();
    let media_base = home.join("jellyfin/media");

    scan_local_category(
        &media_base,
        "movies",
        MediaCategory::Movie,
        &mut items,
        &mut found_keys,
        cache,
    );
    scan_local_category(
        &media_base,
        "shows",
        MediaCategory::Show,
        &mut items,
        &mut found_keys,
        cache,
    );
    scan_local_category(
        &media_base,
        "anime",
        MediaCategory::Anime,
        &mut items,
        &mut found_keys,
        cache,
    );

    cache.local_items.retain(|k, _| found_keys.contains(k));
    tag_watched_local(&mut items);
    items.sort_by_key(|a| a.title.to_lowercase());
    items
}

fn scan_local_category(
    base: &Path,
    folder: &str,
    cat: MediaCategory,
    items: &mut Vec<MediaItem>,
    found_keys: &mut HashSet<String>,
    cache: &mut MediaScanCache,
) {
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

        found_keys.insert(format!("{}/{}", cat.as_str(), name));
        let remote_rel = format!("media/{}/{}", cat.remote_folder(), name);
        let (seasons, files, size) = if cat == MediaCategory::Movie {
            let (f, s) = cache.get_or_scan_movie(&path, &name, cat, true);
            (Vec::new(), f, s)
        } else {
            let ssn = cache.get_or_scan_seasons(&path, &remote_rel, true, cat, &name);
            let s: u64 = ssn.iter().map(|s| s.size_bytes).sum();
            (ssn, Vec::new(), s)
        };

        items.push(MediaItem {
            title: name,
            category: cat,
            size_bytes: size,
            is_watched: false,
            local_path: Some(path),
            remote_path: remote_rel,
            seasons,
            files,
            sync_status: SyncStatus::default(),
        });
    }
}

pub fn scan_remote_media(home: &Path, cache: &mut MediaScanCache) -> Result<Vec<MediaItem>> {
    let mut items = Vec::new();
    let mut found_keys = HashSet::new();
    let gdrive_media = home.join("gdrive/media");

    if gdrive_media.exists() {
        scan_remote_mounted(
            &gdrive_media,
            "movie",
            MediaCategory::Movie,
            &mut items,
            &mut found_keys,
            cache,
        );
        scan_remote_mounted(
            &gdrive_media,
            "anime",
            MediaCategory::Anime,
            &mut items,
            &mut found_keys,
            cache,
        );
        scan_remote_mounted(
            &gdrive_media,
            "anime/movie",
            MediaCategory::Anime,
            &mut items,
            &mut found_keys,
            cache,
        );
        scan_remote_mounted(
            &gdrive_media,
            "shows",
            MediaCategory::Show,
            &mut items,
            &mut found_keys,
            cache,
        );
        cache.remote_items.retain(|k, _| found_keys.contains(k));
    } else {
        scan_remote_via_rclone("media/movie", MediaCategory::Movie, &mut items)?;
        scan_remote_via_rclone("media/anime", MediaCategory::Anime, &mut items)?;
        scan_remote_via_rclone("media/anime/movie", MediaCategory::Anime, &mut items)?;
    }

    items.sort_by_key(|a| a.title.to_lowercase());
    Ok(items)
}

fn scan_remote_mounted(
    base: &Path,
    folder: &str,
    cat: MediaCategory,
    items: &mut Vec<MediaItem>,
    found_keys: &mut HashSet<String>,
    cache: &mut MediaScanCache,
) {
    let folder_path = base.join(folder);
    let Ok(entries) = fs::read_dir(folder_path) else {
        return;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || (folder == "anime" && name == "movie") {
            continue;
        }

        found_keys.insert(format!("{}/{}", cat.as_str(), name));
        let remote_path = format!("media/{folder}/{name}");
        let (seasons, files, size) = if cat == MediaCategory::Movie {
            let (f, s) = cache.get_or_scan_movie(&entry.path(), &name, cat, false);
            (Vec::new(), f, s)
        } else {
            let ssn = cache.get_or_scan_seasons(&entry.path(), &remote_path, false, cat, &name);
            let s: u64 = ssn.iter().map(|s| s.size_bytes).sum();
            (ssn, Vec::new(), s)
        };

        items.push(MediaItem {
            title: name,
            category: cat,
            size_bytes: size,
            is_watched: false,
            local_path: None,
            remote_path,
            seasons,
            files,
            sync_status: SyncStatus::default(),
        });
    }
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
            files: Vec::new(),
            sync_status: SyncStatus::default(),
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
