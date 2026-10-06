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
    let mut missing = Vec::new();
    for path in &tracked_files {
        if path.exists() {
            local_paths.push(path.clone());
        } else {
            missing.push(path.clone());
        }
    }

    if !local_paths.is_empty() {
        // Complete a partial set: pull any missing variants back from Drive so
        // the library is whole instead of silently shipping one variant.
        let pairs: Vec<(PathBuf, PathBuf)> = missing
            .iter()
            .filter_map(|p| find_in_google_drive(p).map(|cloud| (cloud, p.clone())))
            .collect();
        if !pairs.is_empty() {
            let _ = restore_from_cloud(&pairs);
            for (_, dest) in &pairs {
                if dest.exists() {
                    local_paths.push(dest.clone());
                }
            }
        }
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

/// Google Drive mount root.
///
/// Honors `RYOIKI_GDRIVE_DIR` so tests can point at a sandbox tree instead of
/// the real mounted drive.
fn gdrive_root() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("RYOIKI_GDRIVE_DIR") {
        return Some(PathBuf::from(dir));
    }
    let home = std::env::var("HOME").ok()?;
    Some(Path::new(&home).join("gdrive"))
}

#[must_use]
pub fn find_in_google_drive(local_path: &Path) -> Option<PathBuf> {
    let gdrive = gdrive_root()?;
    if !gdrive.exists() {
        return None;
    }
    let rel = extract_rel_media_subpath(local_path)?;
    find_under_base(&gdrive.join("media"), &rel)
}

/// Resolves a media-relative path under an archive base, tolerating the
/// `movies/` ↔ `movie/` category remap.
fn find_under_base(base: &Path, rel: &Path) -> Option<PathBuf> {
    let direct = base.join(rel);
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
mod tests;
