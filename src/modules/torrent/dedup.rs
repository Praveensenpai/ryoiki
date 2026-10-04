//! Deduplication and cloud restoration based on exact magnet-to-file mapping.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Local {
        paths: Vec<PathBuf>,
        title: String,
    },
    Cloud {
        pairs: Vec<(PathBuf, PathBuf)>,
        title: String,
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

/// Checks whether media for this exact torrent magnet is available locally or in Google Drive.
#[must_use]
pub fn check_already_available(magnet_or_url: &str) -> Availability {
    let (hash, dn) = parse_magnet(magnet_or_url);
    let Some(ref h) = hash else {
        return Availability::NotAvailable {
            hash: None,
            display_name: dn,
        };
    };

    let history = super::history::load_history();
    let Some(record) = history.entries.get(h) else {
        return Availability::NotAvailable {
            hash,
            display_name: dn,
        };
    };

    let tracked_files: Vec<PathBuf> = if record.files.is_empty() {
        record.paths.clone()
    } else {
        record.files.iter().map(|f| f.path.clone()).collect()
    };

    if tracked_files.is_empty() {
        return Availability::NotAvailable {
            hash,
            display_name: dn,
        };
    }

    let mut local_paths = Vec::new();
    for path in &tracked_files {
        if path.exists() {
            local_paths.push(path.clone());
        }
    }

    if !local_paths.is_empty() {
        return Availability::Local {
            paths: local_paths,
            title: record.title.clone(),
        };
    }

    let mut cloud_pairs = Vec::new();
    for local_path in &tracked_files {
        if let Some(cloud_path) = find_in_google_drive(local_path) {
            cloud_pairs.push((cloud_path, local_path.clone()));
        }
    }

    if !cloud_pairs.is_empty() {
        return Availability::Cloud {
            pairs: cloud_pairs,
            title: record.title.clone(),
        };
    }

    Availability::NotAvailable {
        hash,
        display_name: dn,
    }
}

#[must_use]
pub fn find_in_google_drive(local_path: &Path) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let gdrive = Path::new(&home).join("gdrive");
    if !gdrive.exists() {
        return None;
    }

    let rel = extract_rel_media_subpath(local_path)?;
    let bases = [gdrive.join("media"), gdrive.join("ryoiki-archive/media")];

    for base in &bases {
        let direct = base.join(&rel);
        if direct.exists() {
            return Some(direct);
        }
        let rel_str = rel.to_str().unwrap_or("");
        if let Some(rest) = rel_str.strip_prefix("movies/") {
            let alt = base.join("movie").join(rest);
            if alt.exists() {
                return Some(alt);
            }
        } else if let Some(rest) = rel_str.strip_prefix("movie/") {
            let alt = base.join("movies").join(rest);
            if alt.exists() {
                return Some(alt);
            }
        }
    }

    None
}

#[must_use]
pub fn extract_rel_media_subpath(path: &Path) -> Option<PathBuf> {
    let path_str = path.to_str()?;
    for marker in &["jellyfin/media/", "media/"] {
        if let Some(idx) = path_str.find(marker) {
            let sub = &path_str[idx + marker.len()..];
            if !sub.is_empty() {
                return Some(PathBuf::from(sub));
            }
        }
    }
    None
}

/// Restores files from Google Drive to local Jellyfin media destination paths.
///
/// # Errors
/// Returns an error if directory creation or file copying fails.
pub fn restore_from_cloud(pairs: &[(PathBuf, PathBuf)]) -> Result<Vec<PathBuf>> {
    let mut restored = Vec::new();
    for (src, dest) in pairs {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent dir {}", parent.display()))?;
        }
        fs::copy(src, dest).with_context(|| {
            format!(
                "Failed to copy from {} to {}",
                src.display(),
                dest.display()
            )
        })?;
        restored.push(dest.clone());
    }

    let _ = crate::modules::jellyfin::api::refresh_library_auto();
    Ok(restored)
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
    fn test_extract_rel_media_subpath() {
        let path = PathBuf::from("/home/neko/jellyfin/media/anime/Show/Season 01/ep1.mkv");
        let rel = extract_rel_media_subpath(&path);
        assert_eq!(rel, Some(PathBuf::from("anime/Show/Season 01/ep1.mkv")));
    }

    #[test]
    fn test_check_already_available_untracked_is_not_available() {
        let mag = "magnet:?xt=urn:btih:ffffffffffffffffffffffffffffffffffffffff&dn=RandomMovie.mkv";
        let res = check_already_available(mag);
        assert!(matches!(res, Availability::NotAvailable { .. }));
    }

    #[test]
    fn test_restore_from_cloud_dual_versions() -> Result<()> {
        let tmp = std::env::temp_dir().join(format!("ryoiki_test_restore_{}", std::process::id()));
        let cloud_dir = tmp.join("cloud");
        let local_dir = tmp.join("local");
        let _ = fs::create_dir_all(&cloud_dir);
        let _ = fs::create_dir_all(&local_dir);

        let src_orig = cloud_dir.join("Show - S01E01 [1080p].mkv");
        let src_multi = cloud_dir.join("Show - S01E01 [1080p] [Multi].mkv");
        fs::write(&src_orig, "orig_content")?;
        fs::write(&src_multi, "multi_content")?;

        let dest_orig = local_dir.join("Show - S01E01 [1080p].mkv");
        let dest_multi = local_dir.join("Show - S01E01 [1080p] [Multi].mkv");

        let pairs = vec![
            (src_orig.clone(), dest_orig.clone()),
            (src_multi.clone(), dest_multi.clone()),
        ];
        let restored = restore_from_cloud(&pairs)?;
        assert_eq!(restored.len(), 2);
        assert!(dest_orig.exists());
        assert!(dest_multi.exists());
        assert_eq!(fs::read_to_string(&dest_orig)?, "orig_content");
        assert_eq!(fs::read_to_string(&dest_multi)?, "multi_content");

        let _ = fs::remove_dir_all(&tmp);
        Ok(())
    }

    #[test]
    fn test_dedup_full_lifecycle_three_examples() -> Result<()> {
        let diff_magnet =
            "magnet:?xt=urn:btih:9999999999999999999999999999999999999999&dn=Frieren+1080p+HighBitrate.mkv";
        let res = check_already_available(diff_magnet);
        assert!(matches!(res, Availability::NotAvailable { .. }));

        let tmp = std::env::temp_dir().join(format!("ryoiki_lifecycle_{}", std::process::id()));
        let local_dir = tmp.join("jellyfin/media/anime/Frieren/Season 01");
        let _ = fs::create_dir_all(&local_dir);
        let orig_file = local_dir.join("Frieren - S01E01 [1080p].mkv");
        let multi_file = local_dir.join("Frieren - S01E01 [1080p] [Multi].mkv");
        fs::write(&orig_file, "orig_video")?;
        fs::write(&multi_file, "multi_video")?;

        let tracked = vec![
            crate::modules::torrent::history::create_tracked_file(orig_file.clone(), "original"),
            crate::modules::torrent::history::create_tracked_file(multi_file.clone(), "multi"),
        ];
        let info = crate::modules::media::heuristic::classify_media_heuristic(
            "Frieren - S01E01 [1080p].mkv",
        );
        let known_hash = "8888888888888888888888888888888888888888";
        crate::modules::torrent::history::record_download_history(
            Some(known_hash),
            &info,
            tracked,
        )?;

        let known_magnet = format!("magnet:?xt=urn:btih:{known_hash}&dn=Frieren+S01E01.mkv");
        let res_local = check_already_available(&known_magnet);
        match res_local {
            Availability::Local { paths, title } => {
                assert_eq!(title, "Frieren");
                assert_eq!(paths.len(), 2);
                assert!(paths.contains(&orig_file));
                assert!(paths.contains(&multi_file));
            }
            _ => panic!("Expected Availability::Local, got {res_local:?}"),
        }

        let _ = fs::remove_file(&orig_file);
        let _ = fs::remove_file(&multi_file);

        let gdrive_dir = tmp.join("gdrive/ryoiki-archive/media/anime/Frieren/Season 01");
        let _ = fs::create_dir_all(&gdrive_dir);
        let gdrive_orig = gdrive_dir.join("Frieren - S01E01 [1080p].mkv");
        let gdrive_multi = gdrive_dir.join("Frieren - S01E01 [1080p] [Multi].mkv");
        fs::write(&gdrive_orig, "cloud_orig_video")?;
        fs::write(&gdrive_multi, "cloud_multi_video")?;

        let pairs = vec![
            (gdrive_orig.clone(), orig_file.clone()),
            (gdrive_multi.clone(), multi_file.clone()),
        ];
        let restored = restore_from_cloud(&pairs)?;
        assert_eq!(restored.len(), 2);
        assert!(orig_file.exists());
        assert!(multi_file.exists());
        assert_eq!(fs::read_to_string(&orig_file)?, "cloud_orig_video");
        assert_eq!(fs::read_to_string(&multi_file)?, "cloud_multi_video");

        let _ = fs::remove_dir_all(&tmp);
        Ok(())
    }
}
