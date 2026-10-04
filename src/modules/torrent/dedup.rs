use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::modules::media::organizer::calculate_dest_dir;
use crate::modules::media::{MediaInfo, MediaType};

pub use super::history::record_download_history;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Local {
        path: PathBuf,
        title: String,
    },
    Cloud {
        path: PathBuf,
        title: String,
        media_type: MediaType,
    },
    NotAvailable {
        hash: Option<String>,
        display_name: Option<String>,
    },
}

/// Extracts the BTIH hash and display name (`dn=`) from a magnet URI or info-hash.
#[must_use]
pub fn parse_magnet(target: &str) -> (Option<String>, Option<String>) {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return (None, None);
    }

    let hash = if trimmed.len() == 40 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(trimmed.to_lowercase())
    } else {
        super::seedr::extract_btih_hash(trimmed)
    };

    let dn = if let Some(idx) = trimmed.find("dn=") {
        let sub = &trimmed[idx + 3..];
        let end = sub.find('&').unwrap_or(sub.len());
        let raw = &sub[..end];
        Some(url_decode(raw))
    } else if !trimmed.starts_with("magnet:")
        && Path::new(trimmed)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("mkv") || ext.eq_ignore_ascii_case("mp4"))
    {
        Some(trimmed.to_string())
    } else {
        None
    };

    (hash, dn)
}

fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            if let (Some(c1), Some(c2)) = (bytes.next(), bytes.next()) {
                if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&[c1, c2]).unwrap_or(""), 16)
                {
                    out.push(v as char);
                }
            }
        } else if b == b'+' {
            out.push(' ');
        } else {
            out.push(b as char);
        }
    }
    out
}

/// Checks whether media is already available locally in Jellyfin or in Google Drive.
pub fn check_already_available(magnet_or_url: &str) -> Availability {
    let (hash, dn) = parse_magnet(magnet_or_url);

    if let Some(ref h) = hash {
        let history = super::history::load_history();
        if let Some(record) = history.entries.get(h) {
            for p in &record.paths {
                if p.exists() {
                    return Availability::Local {
                        path: p.clone(),
                        title: record.title.clone(),
                    };
                }
            }
        }
    }

    let search_name = match (&dn, &hash) {
        (Some(name), _) => name.as_str(),
        (None, Some(h)) => h.as_str(),
        (None, None) => magnet_or_url,
    };

    let info = crate::modules::media::heuristic::classify_media_heuristic(search_name);
    if info.title.trim().is_empty() {
        return Availability::NotAvailable {
            hash,
            display_name: dn,
        };
    }

    if let Some(local_path) = find_in_local_library(&info) {
        return Availability::Local {
            path: local_path,
            title: info.title,
        };
    }

    if let Some(cloud_path) = find_in_google_drive(&info) {
        return Availability::Cloud {
            path: cloud_path,
            title: info.title,
            media_type: info.media_type,
        };
    }

    Availability::NotAvailable {
        hash,
        display_name: dn,
    }
}

fn find_in_local_library(info: &MediaInfo) -> Option<PathBuf> {
    let local_dest_dir = calculate_dest_dir(info);
    if let Some(local_path) = find_existing_in_dir(&local_dest_dir, info) {
        return Some(local_path);
    }

    let home = std::env::var("HOME").ok()?;
    let base = Path::new(&home).join("jellyfin/media");
    for cat in &["anime", "shows", "movies", "movie"] {
        let cat_dir = base.join(cat);
        if let Some(found) = find_in_category_dir(&cat_dir, info) {
            return Some(found);
        }
    }
    None
}

fn find_in_category_dir(cat_dir: &Path, info: &MediaInfo) -> Option<PathBuf> {
    if !cat_dir.exists() {
        return None;
    }
    let target = info
        .year
        .map_or_else(|| info.title.clone(), |y| format!("{} ({y})", info.title));
    for name in &[&target, &info.title] {
        let p = cat_dir.join(name);
        if p.exists() {
            if let Some(found) = find_existing_in_dir(&p, info) {
                return Some(found);
            }
        }
    }
    if let Ok(entries) = fs::read_dir(cat_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.eq_ignore_ascii_case(&info.title) || name.starts_with(&info.title) {
                    if let Some(found) = find_existing_in_dir(&p, info) {
                        return Some(found);
                    }
                }
            }
        }
    }
    None
}

fn find_existing_in_dir(dir: &Path, info: &MediaInfo) -> Option<PathBuf> {
    if !dir.exists() {
        return None;
    }

    if let Some(season) = info.season {
        let sdir = dir.join(format!("Season {season:02}"));
        if sdir.exists() {
            if let Some(p) = find_existing_in_dir(&sdir, info) {
                return Some(p);
            }
        }
    }

    let entries = fs::read_dir(dir).ok()?;
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() && crate::modules::media::organizer::is_video_file(&p) {
            files.push(p);
        }
    }

    if files.is_empty() {
        return None;
    }

    let res = info.resolution.as_deref();
    if let Some(ep) = info.episode {
        let e_pad = format!("E{ep:02}");
        let e_raw = format!("E{ep}");
        let h_pad = format!(" - {ep:02} ");
        let h_raw = format!(" - {ep} ");
        for f in &files {
            let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let ep_match = is_boundary_match(name, &e_pad)
                || is_boundary_match(name, &e_raw)
                || name.contains(&h_pad)
                || name.contains(&h_raw);
            if ep_match && matches_resolution(name, res) {
                return Some(f.clone());
            }
        }
        None
    } else {
        files
            .into_iter()
            .find(|f| matches_resolution(f.file_name().and_then(|n| n.to_str()).unwrap_or(""), res))
    }
}

fn matches_resolution(name: &str, target_res: Option<&str>) -> bool {
    let Some(target) = target_res else {
        return true;
    };
    let Some(existing) = crate::modules::media::heuristic::extract_resolution(name) else {
        return true;
    };
    target.eq_ignore_ascii_case(&existing)
}

fn is_boundary_match(name: &str, tag: &str) -> bool {
    if let Some(idx) = name.find(tag) {
        let after = &name[idx + tag.len()..];
        return !after.chars().next().is_some_and(|c| c.is_ascii_digit());
    }
    false
}

fn find_in_google_drive(info: &MediaInfo) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let gdrive = Path::new(&home).join("gdrive");
    if !gdrive.exists() {
        return None;
    }

    let candidates = [gdrive.join("media"), gdrive.join("ryoiki-archive/media")];

    for base in &candidates {
        if let Some(found) = search_gdrive_category(base, info) {
            return Some(found);
        }
    }
    None
}

fn search_gdrive_category(base: &Path, info: &MediaInfo) -> Option<PathBuf> {
    for cat in &["anime", "shows", "movies", "movie", "show", "series"] {
        let cat_dir = base.join(cat);
        if let Some(found) = find_in_category_dir(&cat_dir, info) {
            return Some(found);
        }
    }
    None
}

/// Copies a file or folder from Google Drive into the local Jellyfin media library.
pub fn restore_from_cloud(cloud_path: &Path, info: &MediaInfo) -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME env not set")?;
    let local_base = Path::new(&home).join("jellyfin/media");

    let dest_dir = if let Some(sub) = find_rel_media_subpath(cloud_path) {
        local_base.join(sub)
    } else {
        calculate_dest_dir(info)
    };
    fs::create_dir_all(&dest_dir)?;

    let file_name = cloud_path
        .file_name()
        .and_then(|n| n.to_str())
        .context("Invalid cloud path filename")?;
    let target_dest = dest_dir.join(file_name);

    if cloud_path.is_file() {
        fs::copy(cloud_path, &target_dest)?;
    } else if cloud_path.is_dir() {
        for entry in fs::read_dir(cloud_path)?.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Some(sub_name) = p.file_name() {
                    let _ = fs::copy(&p, dest_dir.join(sub_name));
                }
            }
        }
    }

    let _ = crate::modules::jellyfin::api::refresh_library_auto();
    Ok(target_dest)
}

fn find_rel_media_subpath(cloud_path: &Path) -> Option<PathBuf> {
    let parent = cloud_path.parent()?;
    let path_str = parent.to_str()?;
    for marker in &["media/anime", "media/shows", "media/movies", "media/movie"] {
        if let Some(idx) = path_str.find(marker) {
            let rel = &path_str[idx + "media/".len()..];
            return Some(PathBuf::from(rel));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_magnet_raw_hash() {
        let hash = "0123456789abcdef0123456789abcdef01234567";
        let (parsed_hash, dn) = parse_magnet(hash);
        assert_eq!(parsed_hash.as_deref(), Some(hash));
        assert!(dn.is_none());
    }

    #[test]
    fn test_parse_magnet_with_dn() {
        let mag = "magnet:?xt=urn:btih:3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c&dn=Sousou%20no%20Frieren%20-%2001%20%5B1080p%5D.mkv";
        let (parsed_hash, dn) = parse_magnet(mag);
        assert_eq!(
            parsed_hash.as_deref(),
            Some("3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c")
        );
        assert_eq!(dn.as_deref(), Some("Sousou no Frieren - 01 [1080p].mkv"));
    }

    #[test]
    fn test_url_decode() {
        assert_eq!(url_decode("Hello%20World%2BTest"), "Hello World+Test");
        assert_eq!(url_decode("Title%20%5B1080p%5D"), "Title [1080p]");
    }

    #[test]
    fn test_check_already_available_not_available() {
        let mag = "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=NonExistentMovie99999999.mkv";
        let res = check_already_available(mag);
        assert!(matches!(res, Availability::NotAvailable { .. }));
    }

    #[test]
    fn test_matches_resolution() {
        assert!(matches_resolution(
            "Movie (2024) [1080p].mkv",
            Some("1080p")
        ));
        assert!(!matches_resolution(
            "Movie (2024) [720p].mkv",
            Some("1080p")
        ));
        assert!(!matches_resolution(
            "Movie (2024) [1080p].mkv",
            Some("2160p")
        ));
        assert!(matches_resolution("Movie (2024) [1080p].mkv", None));
    }
}
