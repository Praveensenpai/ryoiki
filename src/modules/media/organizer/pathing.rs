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
            let parent = base.join("shows").join(&info.title);
            let season = if info.is_extra {
                info.season.unwrap_or(0)
            } else {
                info.season.unwrap_or(1)
            };
            parent.join(format!("Season {season:02}"))
        }
        MediaType::Anime => {
            let parent = base.join("anime").join(&info.title);
            if info.is_extra {
                let season = info.season.unwrap_or(0);
                parent.join(format!("Season {season:02}"))
            } else if info.season.is_some() || info.episode.is_some() {
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

pub fn resolve_unique_dest_path(
    src: &Path,
    dest_dir: &Path,
    info: &MediaInfo,
    dry_run: bool,
) -> PathBuf {
    let standard = dest_dir.join(&info.clean_name);
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

    #[test]
    fn test_calculate_dest_dir_tv_and_anime_extras() {
        let anime_extra = MediaInfo {
            media_type: MediaType::Anime,
            title: "Shirokuma Cafe".to_string(),
            year: None,
            season: None,
            episode: None,
            resolution: Some("1080p".to_string()),
            language: Some("Japanese".to_string()),
            clean_name: "Shirokuma Cafe - S00E01 - Menu 01 [Japanese] [1080p].mkv".to_string(),
            is_extra: true,
            engine: ClassificationEngine::Ai,
        };
        let dest = calculate_dest_dir(&anime_extra);
        assert!(dest.ends_with("anime/Shirokuma Cafe/Season 00"));

        let anime_regular = MediaInfo {
            media_type: MediaType::Anime,
            title: "Shirokuma Cafe".to_string(),
            year: None,
            season: Some(1),
            episode: Some(1),
            resolution: Some("1080p".to_string()),
            language: Some("Japanese".to_string()),
            clean_name: "Shirokuma Cafe - S01E01 [Japanese] [1080p].mkv".to_string(),
            is_extra: false,
            engine: ClassificationEngine::Ai,
        };
        let dest = calculate_dest_dir(&anime_regular);
        assert!(dest.ends_with("anime/Shirokuma Cafe/Season 01"));

        let movie_extra = MediaInfo {
            media_type: MediaType::Movie,
            title: "Inception".to_string(),
            year: Some(2010),
            season: None,
            episode: None,
            resolution: Some("1080p".to_string()),
            language: Some("English".to_string()),
            clean_name: "Inception (2010) [English] [1080p].mkv".to_string(),
            is_extra: true,
            engine: ClassificationEngine::Ai,
        };
        let dest = calculate_dest_dir(&movie_extra);
        assert!(dest.ends_with("movies/Inception (2010)/extras"));
    }
}
