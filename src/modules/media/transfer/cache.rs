use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::scan::collect_dir_files;
use super::{MediaCategory, MediaFile, MediaSeason, SyncStatus};

const CACHE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedSeason {
    pub title: String,
    pub mtime_secs: u64,
    pub size_bytes: u64,
    pub files: Vec<MediaFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CachedItem {
    pub title: String,
    pub category: MediaCategory,
    pub mtime_secs: u64,
    pub size_bytes: u64,
    pub files: Vec<MediaFile>,
    pub seasons: Vec<CachedSeason>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MediaScanCache {
    pub version: u32,
    pub local_items: HashMap<String, CachedItem>,
    pub remote_items: HashMap<String, CachedItem>,
}

pub fn path_mtime_secs(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

pub fn cache_file_path(home: &Path) -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map_or_else(|| home.join(".cache"), PathBuf::from)
        .join("ryoiki/media_cache.json")
}

pub fn load_cache(home: &Path) -> MediaScanCache {
    let path = cache_file_path(home);
    let Ok(data) = fs::read_to_string(&path) else {
        return MediaScanCache {
            version: CACHE_VERSION,
            ..Default::default()
        };
    };

    serde_json::from_str::<MediaScanCache>(&data).unwrap_or_else(|_| MediaScanCache {
        version: CACHE_VERSION,
        ..Default::default()
    })
}

impl MediaScanCache {
    pub fn save(&self, home: &Path) -> Result<()> {
        let path = cache_file_path(home);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(self)?;
        fs::write(&path, json)?;
        Ok(())
    }

    pub fn invalidate(home: &Path) -> Result<()> {
        let path = cache_file_path(home);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    pub fn get_or_scan_movie(
        &mut self,
        path: &Path,
        title: &str,
        cat: MediaCategory,
        is_local: bool,
    ) -> (Vec<MediaFile>, u64) {
        let key = format!("{}/{}", cat.as_str(), title);
        let mtime = path_mtime_secs(path);
        let cache_map = if is_local {
            &mut self.local_items
        } else {
            &mut self.remote_items
        };

        if let Some(cached) = cache_map.get(&key) {
            if cached.mtime_secs == mtime && mtime > 0 {
                return (cached.files.clone(), cached.size_bytes);
            }
        }

        let files = collect_dir_files(path, "");
        let size: u64 = files.iter().map(|f| f.size_bytes).sum();
        cache_map.insert(
            key,
            CachedItem {
                title: title.to_string(),
                category: cat,
                mtime_secs: mtime,
                size_bytes: size,
                files: files.clone(),
                seasons: Vec::new(),
            },
        );
        (files, size)
    }

    pub fn get_or_scan_seasons(
        &mut self,
        dir_path: &Path,
        remote_base: &str,
        is_local: bool,
        cat: MediaCategory,
        show_title: &str,
    ) -> Vec<MediaSeason> {
        let Ok(entries) = fs::read_dir(dir_path) else {
            return Vec::new();
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

        let item_key = format!("{}/{}", cat.as_str(), show_title);
        let cache_map = if is_local {
            &mut self.local_items
        } else {
            &mut self.remote_items
        };

        let cached_seasons_map: HashMap<String, CachedSeason> = cache_map
            .get(&item_key)
            .map(|it| {
                it.seasons
                    .iter()
                    .map(|s| (s.title.clone(), s.clone()))
                    .collect()
            })
            .unwrap_or_default();

        let mut seasons = Vec::new();
        let mut new_cached_seasons = Vec::new();

        for (name, path) in subdirs {
            let mtime = path_mtime_secs(&path);
            let (files, size) = if let Some(cs) = cached_seasons_map.get(&name) {
                if cs.mtime_secs == mtime && mtime > 0 {
                    (cs.files.clone(), cs.size_bytes)
                } else {
                    let f = collect_dir_files(&path, "");
                    let s: u64 = f.iter().map(|it| it.size_bytes).sum();
                    (f, s)
                }
            } else {
                let f = collect_dir_files(&path, "");
                let s: u64 = f.iter().map(|it| it.size_bytes).sum();
                (f, s)
            };

            new_cached_seasons.push(CachedSeason {
                title: name.clone(),
                mtime_secs: mtime,
                size_bytes: size,
                files: files.clone(),
            });

            let remote_path = format!("{remote_base}/{name}");
            seasons.push(MediaSeason {
                title: name,
                size_bytes: size,
                local_path: if is_local { Some(path) } else { None },
                remote_path,
                is_selected: false,
                files,
                sync_status: SyncStatus::default(),
            });
        }

        let total_size = new_cached_seasons.iter().map(|s| s.size_bytes).sum();
        cache_map.insert(
            item_key,
            CachedItem {
                title: show_title.to_string(),
                category: cat,
                mtime_secs: path_mtime_secs(dir_path),
                size_bytes: total_size,
                files: Vec::new(),
                seasons: new_cached_seasons,
            },
        );

        seasons
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_serialization_roundtrip() -> Result<()> {
        let mut cache = MediaScanCache {
            version: 1,
            local_items: HashMap::new(),
            remote_items: HashMap::new(),
        };

        cache.local_items.insert(
            "Movie/Test Movie".to_string(),
            CachedItem {
                title: "Test Movie".to_string(),
                category: MediaCategory::Movie,
                mtime_secs: 1_234_567,
                size_bytes: 1000,
                files: vec![MediaFile {
                    name: "test.mkv".to_string(),
                    rel_path: "test.mkv".to_string(),
                    size_bytes: 1000,
                    exists_in_other: false,
                }],
                seasons: Vec::new(),
            },
        );

        let json = serde_json::to_string(&cache)?;
        let loaded: MediaScanCache = serde_json::from_str(&json)?;
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.local_items.len(), 1);
        if let Some(item) = loaded.local_items.get("Movie/Test Movie") {
            assert_eq!(item.title, "Test Movie");
            assert_eq!(item.size_bytes, 1000);
        } else {
            panic!("Expected item not found in cache");
        }
        Ok(())
    }

    #[test]
    fn test_path_mtime_secs_returns_timestamp() {
        let temp = std::env::temp_dir().join("test_mtime_file.txt");
        let _ = fs::write(&temp, "test");
        let mtime = path_mtime_secs(&temp);
        let _ = fs::remove_file(&temp);
        assert!(mtime > 0);
    }

    #[test]
    fn test_get_or_scan_movie_hits_cache() -> Result<()> {
        let temp_dir = std::env::temp_dir().join("test_movie_cache_dir");
        let _ = fs::create_dir_all(&temp_dir);
        let movie_file = temp_dir.join("test_movie.mkv");
        fs::write(&movie_file, b"sample bytes")?;

        let mut cache = MediaScanCache::default();
        let (files1, size1) =
            cache.get_or_scan_movie(&temp_dir, "Test Movie", MediaCategory::Movie, true);
        assert_eq!(files1.len(), 1);
        assert_eq!(size1, 12);

        // Second call should hit cache directly
        let (files2, size2) =
            cache.get_or_scan_movie(&temp_dir, "Test Movie", MediaCategory::Movie, true);
        assert_eq!(files2.len(), 1);
        assert_eq!(size2, 12);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
