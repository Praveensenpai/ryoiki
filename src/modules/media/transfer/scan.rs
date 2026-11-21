use std::collections::HashSet;
use std::fs;
use std::path::Path;

use super::cache::MediaScanCache;
use super::{MediaCategory, MediaFile, MediaItem, SyncStatus};

/// Both media libraries plus any non-fatal cloud access warning.
#[derive(Debug, Default)]
pub struct LibraryScan {
    pub local: Vec<MediaItem>,
    pub remote: Vec<MediaItem>,
    pub cloud_warning: Option<String>,
}

pub fn collect_dir_files(path: &Path, rel_prefix: &str) -> Vec<MediaFile> {
    let mut files = Vec::new();
    if path.is_file() {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().to_string());
        let size = fs::metadata(path).map_or(0, |m| m.len());
        let rel_path = if rel_prefix.is_empty() {
            name.clone()
        } else {
            format!("{rel_prefix}/{name}")
        };
        return vec![MediaFile {
            name,
            rel_path,
            size_bytes: size,
            exists_in_other: false,
            is_selected: false,
        }];
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
                is_selected: false,
            });
        } else if p.is_dir() {
            files.extend(collect_dir_files(&p, &sub_rel));
        }
    }
    files.sort_by_key(|a| a.name.to_lowercase());
    files
}

pub fn scan_libraries(home: &Path) -> LibraryScan {
    let mut cache = super::cache::load_cache(home);
    let mut local = scan_local_media(home, &mut cache);
    let remote_scan = super::cloud::scan_remote_media(home, &mut cache);
    let mut remote = remote_scan.items;
    super::cross_reference_libraries(&mut local, &mut remote);
    let _ = cache.save(home);
    LibraryScan {
        local,
        remote,
        cloud_warning: remote_scan.warning,
    }
}

pub fn rescan_libraries(home: &Path) -> LibraryScan {
    let _ = MediaScanCache::invalidate(home);
    scan_libraries(home)
}

pub fn scan_local_media(home: &Path, cache: &mut MediaScanCache) -> Vec<MediaItem> {
    let mut items = Vec::new();
    let mut found_keys = HashSet::new();
    let media_base = home.join("jellyfin/media");

    let targets = [
        ("movies", MediaCategory::Movie),
        ("shows", MediaCategory::Show),
        ("anime", MediaCategory::Anime),
        ("anime/movie", MediaCategory::Anime),
        ("anime/movies", MediaCategory::Anime),
    ];
    for (folder, cat) in targets {
        if media_base.join(folder).is_dir() {
            scan_local_category(&media_base, folder, cat, &mut items, &mut found_keys, cache);
        }
    }

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
        if name.starts_with('.') || (folder == "anime" && (name == "movie" || name == "movies")) {
            continue;
        }

        let item_key = format!("{}/{}", cat.as_str(), name);
        if found_keys.contains(&item_key) {
            continue;
        }
        found_keys.insert(item_key);
        let remote_rel = format!("media/{}/{name}", remote_subpath(folder));
        let (seasons, files, size) = if cat == MediaCategory::Movie {
            let (f, s) = cache.get_or_scan_movie(&path, &name, cat, true);
            (Vec::new(), f, s)
        } else {
            let ssn = cache.get_or_scan_seasons(&path, &remote_rel, true, cat, &name);
            if ssn.is_empty() {
                let (f, s) = cache.get_or_scan_movie(&path, &name, cat, true);
                (Vec::new(), f, s)
            } else {
                let s: u64 = ssn.iter().map(|s| s.size_bytes).sum();
                (ssn, Vec::new(), s)
            }
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

fn tag_watched_local(items: &mut [MediaItem]) {
    let url = "http://localhost:8096/Items?Filters=IsPlayed&Recursive=true&Fields=Path";
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build();
    let Ok(resp) = client.and_then(|c| c.get(url).send()) else {
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

/// Maps a local media folder to its Google Drive counterpart.
///
/// Mirrors the daily sync layout: `movies` is stored as `movie` (singular),
/// so every archive/sync path resolves to a single cloud tree.
fn remote_subpath(folder: &str) -> String {
    match folder.split_once('/') {
        Some((head, tail)) => format!("{}/{tail}", map_head(head)),
        None => map_head(folder).to_string(),
    }
}

fn map_head(head: &str) -> &str {
    match head {
        "movies" => "movie",
        other => other,
    }
}

#[cfg(test)]
mod tests;
