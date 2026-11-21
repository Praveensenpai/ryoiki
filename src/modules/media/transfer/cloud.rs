//! Remote Google Drive library scanning.
//!
//! The live rclone mount is preferred because it is faster than repeated API
//! calls. When the mount is down, the `gdrive:` remote is listed directly.
//! Failures become an actionable warning instead of being dropped, so the
//! media manager can tell an empty library from an unreachable cloud.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use super::cache::MediaScanCache;
use super::{MediaCategory, MediaItem, SyncStatus};
use crate::modules::rclone::describe_error;

/// One remote library scan with an optional cloud access warning.
#[derive(Debug, Default)]
pub struct RemoteScan {
    pub items: Vec<MediaItem>,
    pub warning: Option<String>,
}

const TARGETS: [(&str, &str, MediaCategory); 4] = [
    ("movie", "media/movie", MediaCategory::Movie),
    ("shows", "media/shows", MediaCategory::Show),
    ("anime", "media/anime", MediaCategory::Anime),
    ("anime/movie", "media/anime/movie", MediaCategory::Anime),
];

/// Scans the cloud library, preferring the mounted tree when it is live.
pub fn scan_remote_media(home: &Path, cache: &mut MediaScanCache) -> RemoteScan {
    let gdrive_root = home.join("gdrive");
    let gdrive_media = gdrive_root.join("media");

    let mut scan = if is_active_mount(&gdrive_root) && gdrive_media.exists() {
        scan_mounted_libraries(&gdrive_media, cache)
    } else {
        scan_rclone_libraries()
    };

    scan.items.sort_by_key(|a| a.title.to_lowercase());
    scan
}

/// Returns true only when `path` is an actual mount point.
///
/// `Path::exists()` cannot tell an empty mount point from an unmounted
/// directory, which makes an unmounted cloud silently look empty.
#[must_use]
pub fn is_active_mount(path: &Path) -> bool {
    Command::new("findmnt")
        .args(["-n", "-o", "TARGET", "--"])
        .arg(path)
        .output()
        .is_ok_and(|out| {
            out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty()
        })
}

fn scan_mounted_libraries(base: &Path, cache: &mut MediaScanCache) -> RemoteScan {
    let mut scan = MountedScan {
        base,
        cache,
        items: Vec::new(),
        found_keys: HashSet::new(),
    };

    for (local_sub, _, cat) in TARGETS {
        scan.scan_category(local_sub, cat);
    }

    let MountedScan {
        cache,
        items,
        found_keys,
        ..
    } = scan;
    cache.remote_items.retain(|k, _| found_keys.contains(k));

    RemoteScan {
        items,
        warning: None,
    }
}

struct MountedScan<'a> {
    base: &'a Path,
    cache: &'a mut MediaScanCache,
    items: Vec<MediaItem>,
    found_keys: HashSet<String>,
}

impl MountedScan<'_> {
    fn scan_category(&mut self, folder: &str, cat: MediaCategory) {
        let folder_path = self.base.join(folder);
        let Ok(entries) = fs::read_dir(folder_path) else {
            return;
        };

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.')
                || (folder == "anime" && matches!(name.as_str(), "movie" | "movies"))
            {
                continue;
            }

            let item_key = format!("{}/{}", cat.as_str(), name);
            if !self.found_keys.insert(item_key) {
                continue;
            }

            let remote_path = format!("media/{folder}/{name}");
            let (seasons, files, size) = if cat == MediaCategory::Movie {
                let (f, s) = self
                    .cache
                    .get_or_scan_movie(&entry.path(), &name, cat, false);
                (Vec::new(), f, s)
            } else {
                let ssn =
                    self.cache
                        .get_or_scan_seasons(&entry.path(), &remote_path, false, cat, &name);
                if ssn.is_empty() {
                    let (f, s) = self
                        .cache
                        .get_or_scan_movie(&entry.path(), &name, cat, false);
                    (Vec::new(), f, s)
                } else {
                    let s: u64 = ssn.iter().map(|s| s.size_bytes).sum();
                    (ssn, Vec::new(), s)
                }
            };

            self.items.push(MediaItem {
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
}

fn scan_rclone_libraries() -> RemoteScan {
    let mut items = Vec::new();
    let mut warning = None;

    for (_, remote_sub, cat) in TARGETS {
        match list_remote_dirs(remote_sub) {
            Ok(names) => append_remote_items(remote_sub, cat, &names, &mut items),
            Err(raw) if is_missing_dir(&raw) => {}
            Err(raw) => {
                if warning.is_none() {
                    warning = Some(describe_error(&raw));
                }
                if is_fatal_cloud_error(&raw) {
                    break;
                }
            }
        }
    }

    RemoteScan { items, warning }
}

fn list_remote_dirs(remote_dir: &str) -> Result<Vec<String>, String> {
    let out = Command::new("rclone")
        .args(["lsf", "--dirs-only", &format!("gdrive:{remote_dir}")])
        .output()
        .map_err(|err| format!("rclone not available: {err}"))?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|line| line.trim().trim_end_matches('/').to_string())
        .filter(|name| !name.is_empty())
        .collect())
}

fn append_remote_items(
    remote_dir: &str,
    cat: MediaCategory,
    names: &[String],
    items: &mut Vec<MediaItem>,
) {
    for name in names {
        if remote_dir.ends_with("anime") && matches!(name.as_str(), "movie" | "movies") {
            continue;
        }
        items.push(MediaItem {
            title: name.clone(),
            category: cat,
            size_bytes: 0,
            is_watched: false,
            local_path: None,
            remote_path: format!("{remote_dir}/{name}"),
            seasons: Vec::new(),
            files: Vec::new(),
            sync_status: SyncStatus::default(),
        });
    }
}

/// A missing category directory is a valid empty cloud library, not an error.
fn is_missing_dir(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    lower.contains("directory not found") || lower.contains("does not exist")
}

/// Errors that will repeat for every category, so scanning can stop early.
fn is_fatal_cloud_error(raw: &str) -> bool {
    if is_missing_dir(raw) {
        return false;
    }
    let lower = raw.to_ascii_lowercase();
    [
        "invalid_grant",
        "couldn't fetch token",
        "failed to create file system",
        "no such remote",
        "didn't find section",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_directory_is_not_fatal() {
        let raw = "CRITICAL: Failed to create file system for \"gdrive:media/movie\": directory not found";
        assert!(is_missing_dir(raw));
        assert!(!is_fatal_cloud_error(raw));
    }

    #[test]
    fn expired_token_is_fatal() {
        let raw = "CRITICAL: couldn't fetch token: invalid_grant: maybe token expired?";
        assert!(is_fatal_cloud_error(raw));
    }

    #[test]
    fn root_is_a_mount_point() {
        assert!(is_active_mount(Path::new("/")));
    }

    #[test]
    fn ordinary_directory_is_not_a_mount_point() {
        let dir = std::env::temp_dir().join(format!("ryoiki-mount-{}", std::process::id()));
        assert!(fs::create_dir_all(&dir).is_ok(), "create temp dir");
        assert!(!is_active_mount(&dir));
        let _ = fs::remove_dir_all(&dir);
    }
}
