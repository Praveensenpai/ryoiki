use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::modules::media::{MediaInfo, MediaType};

const VIDEO_EXTENSIONS: [&str; 8] = ["mkv", "mp4", "avi", "mov", "wmv", "m4v", "webm", "ts"];

pub fn is_video_file(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    if path_str.ends_with(".!qB") || path_str.ends_with(".!qb") {
        return false;
    }
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

pub fn get_jellyfin_media_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    Path::new(&home).join("jellyfin/media")
}

pub fn calculate_dest_dir(info: &MediaInfo) -> PathBuf {
    let base = get_jellyfin_media_dir();
    match info.media_type {
        MediaType::Movie => {
            let folder = info
                .year
                .map_or_else(|| info.title.clone(), |y| format!("{} ({y})", info.title));
            let movie_dir = base.join("movies").join(folder);
            if info.is_extra {
                movie_dir.join("extras")
            } else {
                movie_dir
            }
        }
        MediaType::Show => {
            let parent = resolve_series_dir(&base.join("shows"), &info.title);
            let season = if info.is_extra {
                info.season.unwrap_or(0)
            } else {
                info.season.unwrap_or(1)
            };
            parent.join(format!("Season {season:02}"))
        }
        MediaType::Anime => {
            if info.is_extra {
                let parent = resolve_series_dir(&base.join("anime"), &info.title);
                let season = info.season.unwrap_or(0);
                parent.join(format!("Season {season:02}"))
            } else if info.season.is_some() || info.episode.is_some() {
                let parent = resolve_series_dir(&base.join("anime"), &info.title);
                parent.join(format!("Season {:02}", info.season.unwrap_or(1)))
            } else {
                let folder = info
                    .year
                    .map_or_else(|| info.title.clone(), |y| format!("{} ({y})", info.title));
                base.join("anime").join(folder)
            }
        }
    }
}

pub fn resolve_series_dir(parent_category: &Path, title: &str) -> PathBuf {
    let exact_dir = parent_category.join(title);
    if exact_dir.exists() {
        return exact_dir;
    }

    if let Ok(entries) = fs::read_dir(parent_category) {
        let mut candidates: Vec<PathBuf> = Vec::new();
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(folder_name) = entry.file_name().to_str() {
                    if is_franchise_prefix(title, folder_name) {
                        candidates.push(entry.path());
                    }
                }
            }
        }
        if let Some(best) = candidates
            .into_iter()
            .max_by_key(|p| p.to_string_lossy().len())
        {
            return best;
        }
    }

    exact_dir
}

fn is_franchise_prefix(title: &str, folder_name: &str) -> bool {
    if folder_name.is_empty() || title.len() <= folder_name.len() {
        return false;
    }

    if title
        .to_ascii_lowercase()
        .starts_with(&folder_name.to_ascii_lowercase())
    {
        let next_char = title[folder_name.len()..].chars().next();
        return matches!(next_char, Some(' ' | '-' | ':' | '_'));
    }

    false
}

pub fn resolve_unique_dest_path(
    src: &Path,
    dest_dir: &Path,
    info: &MediaInfo,
    dry_run: bool,
) -> PathBuf {
    let clean_name = canonicalize_filename_for_dest(dest_dir, info);
    let standard = dest_dir.join(&clean_name);
    if !standard.exists() {
        return standard;
    }

    let src_size = fs::metadata(src).map_or(0, |m| m.len());
    let dst_size = fs::metadata(&standard).map_or(0, |m| m.len());

    if src_size.abs_diff(dst_size) < 1024 {
        return standard;
    }

    let ext = standard
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mkv");
    let stem = standard
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&info.clean_name);
    let size_tag = crate::modules::torrent::notify::format_size(src_size).replace(' ', "");
    let append_tag = |s: &str, tag: &str| {
        s.strip_suffix(']').map_or_else(
            || format!("{s} - [{tag}].{ext}"),
            |p| format!("{p} - {tag}].{ext}"),
        )
    };

    let new_name = append_tag(stem, &size_tag);
    if !dry_run && !stem.contains(" - ") {
        let old_size_tag = crate::modules::torrent::notify::format_size(dst_size).replace(' ', "");
        let _ = fs::rename(&standard, dest_dir.join(append_tag(stem, &old_size_tag)));
    }

    dest_dir.join(new_name)
}

fn canonicalize_filename_for_dest(dest_dir: &Path, info: &MediaInfo) -> String {
    let is_season_folder = dest_dir
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| name.starts_with("Season ") || name.eq_ignore_ascii_case("extras"));

    if is_season_folder {
        if let Some(parent_series) = dest_dir
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
        {
            if !parent_series.is_empty()
                && !parent_series.eq_ignore_ascii_case(&info.title)
                && info
                    .clean_name
                    .to_ascii_lowercase()
                    .starts_with(&info.title.to_ascii_lowercase())
            {
                let suffix = &info.clean_name[info.title.len()..];
                return format!("{parent_series}{suffix}");
            }
        }
    }

    info.clean_name.clone()
}

pub fn perform_move(src: &Path, dst: &Path) -> Result<()> {
    if fs::rename(src, dst).is_ok() {
        return Ok(());
    }

    fs::copy(src, dst)
        .with_context(|| format!("Failed to copy {} to {}", src.display(), dst.display()))?;
    fs::remove_file(src)
        .with_context(|| format!("Failed to remove source file {}", src.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::media::ClassificationEngine;

    #[test]
    fn test_is_video_file_extensions() {
        assert!(is_video_file(Path::new("movie.mkv")));
        assert!(is_video_file(Path::new("show.mp4")));
        assert!(is_video_file(Path::new("anime.webm")));
        assert!(!is_video_file(Path::new("subtitles.srt")));
        assert!(!is_video_file(Path::new("image.png")));
    }

    #[test]
    fn test_is_video_file_skips_qb_incomplete() {
        assert!(!is_video_file(Path::new("movie.mkv.!qB")));
        assert!(!is_video_file(Path::new("movie.mp4.!qb")));
    }

    fn mock_info(title: &str, clean_name: &str, season: Option<u32>) -> MediaInfo {
        MediaInfo {
            media_type: MediaType::Anime,
            title: title.to_string(),
            year: None,
            season,
            episode: Some(1),
            resolution: Some("1080p".to_string()),
            language: Some("Japanese".to_string()),
            clean_name: clean_name.to_string(),
            is_extra: false,
            engine: ClassificationEngine::Ai,
        }
    }

    #[test]
    fn test_canonicalize_filename_for_dest() {
        let info = mock_info(
            "Non Non Biyori Repeat",
            "Non Non Biyori Repeat - S02E01 [Japanese] [1080p].mkv",
            Some(2),
        );

        let season_dest = Path::new("/media/anime/Non Non Biyori/Season 02");
        assert_eq!(
            canonicalize_filename_for_dest(season_dest, &info),
            "Non Non Biyori - S02E01 [Japanese] [1080p].mkv"
        );

        let standalone_dest = Path::new("/media/anime/Non Non Biyori Repeat");
        assert_eq!(
            canonicalize_filename_for_dest(standalone_dest, &info),
            "Non Non Biyori Repeat - S02E01 [Japanese] [1080p].mkv"
        );
    }

    #[test]
    fn test_resolve_series_dir_franchise_prefix() {
        let temp_dir =
            std::env::temp_dir().join(format!("ryoiki_test_anime_{}", std::process::id()));
        let _ = fs::create_dir_all(temp_dir.join("Non Non Biyori"));

        assert_eq!(
            resolve_series_dir(&temp_dir, "Non Non Biyori Repeat"),
            temp_dir.join("Non Non Biyori")
        );
        assert_eq!(
            resolve_series_dir(&temp_dir, "Non Non Biyori Nonstop"),
            temp_dir.join("Non Non Biyori")
        );
        assert_eq!(
            resolve_series_dir(&temp_dir, "Demon Slayer"),
            temp_dir.join("Demon Slayer")
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_calculate_dest_dir_tv_and_anime_extras() {
        let mut anime_extra = mock_info(
            "Shirokuma Cafe",
            "Shirokuma Cafe - S00E01 - Menu 01.mkv",
            None,
        );
        anime_extra.is_extra = true;
        assert!(calculate_dest_dir(&anime_extra).ends_with("anime/Shirokuma Cafe/Season 00"));

        let anime_regular = mock_info("Shirokuma Cafe", "Shirokuma Cafe - S01E01.mkv", Some(1));
        assert!(calculate_dest_dir(&anime_regular).ends_with("anime/Shirokuma Cafe/Season 01"));

        let mut movie_extra = mock_info("Inception", "Inception (2010).mkv", None);
        movie_extra.media_type = MediaType::Movie;
        movie_extra.year = Some(2010);
        movie_extra.is_extra = true;
        assert!(calculate_dest_dir(&movie_extra).ends_with("movies/Inception (2010)/extras"));
    }
}
