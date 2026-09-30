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
        let remote_rel = format!("ryoiki-archive/media/{folder}/{name}");
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

pub fn scan_remote_media(home: &Path, cache: &mut MediaScanCache) -> Result<Vec<MediaItem>> {
    let mut items = Vec::new();
    let mut found_keys = HashSet::new();
    let gdrive_media = home.join("gdrive/media");

    let targets = [
        (
            "movies",
            "ryoiki-archive/media/movies",
            MediaCategory::Movie,
        ),
        ("shows", "ryoiki-archive/media/shows", MediaCategory::Show),
        ("anime", "ryoiki-archive/media/anime", MediaCategory::Anime),
        (
            "anime/movie",
            "ryoiki-archive/media/anime/movie",
            MediaCategory::Anime,
        ),
    ];

    if gdrive_media.exists() {
        for (local_sub, _, cat) in targets {
            scan_remote_mounted(
                &gdrive_media,
                local_sub,
                cat,
                &mut items,
                &mut found_keys,
                cache,
            );
        }
        cache.remote_items.retain(|k, _| found_keys.contains(k));
    } else {
        for (_, remote_sub, cat) in targets {
            scan_remote_via_rclone(remote_sub, cat, &mut items)?;
        }
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
        if name.starts_with('.') || (folder == "anime" && (name == "movie" || name == "movies")) {
            continue;
        }

        let item_key = format!("{}/{}", cat.as_str(), name);
        if found_keys.contains(&item_key) {
            continue;
        }
        found_keys.insert(item_key);
        let remote_path = format!("ryoiki-archive/media/{folder}/{name}");
        let (seasons, files, size) = if cat == MediaCategory::Movie {
            let (f, s) = cache.get_or_scan_movie(&entry.path(), &name, cat, false);
            (Vec::new(), f, s)
        } else {
            let ssn = cache.get_or_scan_seasons(&entry.path(), &remote_path, false, cat, &name);
            if ssn.is_empty() {
                let (f, s) = cache.get_or_scan_movie(&entry.path(), &name, cat, false);
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
        if name.is_empty()
            || (remote_dir.ends_with("anime") && (name == "movie" || name == "movies"))
        {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_local_category_flat_anime() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("test_scan_flat_anime");
        let anime_dir = temp_dir.join("anime").join("Kimetsu no Yaiba (2025)");
        fs::create_dir_all(&anime_dir)?;
        let file_path = anime_dir.join("Kimetsu no Yaiba (2025) [1080p].mkv");
        fs::write(&file_path, b"1234567890")?;

        let mut items = Vec::new();
        let mut found_keys = HashSet::new();
        let mut cache = MediaScanCache::default();

        scan_local_category(
            &temp_dir,
            "anime",
            MediaCategory::Anime,
            &mut items,
            &mut found_keys,
            &mut cache,
        );

        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.title, "Kimetsu no Yaiba (2025)");
        assert_eq!(item.category, MediaCategory::Anime);
        assert_eq!(item.size_bytes, 10);
        assert!(item.seasons.is_empty());
        assert_eq!(item.files.len(), 1);
        assert_eq!(item.files[0].name, "Kimetsu no Yaiba (2025) [1080p].mkv");

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_scan_local_anime_movies_subfolder() -> Result<()> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_anime_movies_{}", std::process::id()));
        let anime_movie_dir = temp_dir.join("anime/movie/Koe no Katachi (2016)");
        fs::create_dir_all(&anime_movie_dir)?;
        let file_path = anime_movie_dir.join("Koe no Katachi (2016) [Japanese] [1080p].mkv");
        fs::write(&file_path, b"koe_no_katachi_bytes")?;

        let mut items = Vec::new();
        let mut found_keys = HashSet::new();
        let mut cache = MediaScanCache::default();

        // Scanning "anime" must skip "movie"
        scan_local_category(
            &temp_dir,
            "anime",
            MediaCategory::Anime,
            &mut items,
            &mut found_keys,
            &mut cache,
        );
        assert!(
            items.is_empty(),
            "anime/movie should not be added as an item titled 'movie'"
        );

        // Scanning "anime/movie" must discover Koe no Katachi (2016) directly
        scan_local_category(
            &temp_dir,
            "anime/movie",
            MediaCategory::Anime,
            &mut items,
            &mut found_keys,
            &mut cache,
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Koe no Katachi (2016)");
        assert_eq!(items[0].category, MediaCategory::Anime);
        assert!(items[0].seasons.is_empty());
        assert_eq!(items[0].files.len(), 1);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
