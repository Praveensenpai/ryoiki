//! Read-only inventory of the Jellyfin media library.
//!
//! Counts titles and playable video files per section and reports folders that
//! exist but hold no video file, which Jellyfin cannot index.

use std::fs;
use std::path::{Path, PathBuf};

use super::organizer::pathing::is_video_file;
use crate::notify::client::escape_html;

/// Maximum directory depth searched inside a single title folder.
const MAX_DEPTH: usize = 4;

/// Static description of one library section.
struct SectionSpec {
    label: &'static str,
    emoji: &'static str,
    dir: &'static str,
    unit: &'static str,
}

const SECTIONS: [SectionSpec; 3] = [
    SectionSpec {
        label: "Movies",
        emoji: "🎬",
        dir: "movies",
        unit: "title",
    },
    SectionSpec {
        label: "Shows",
        emoji: "📺",
        dir: "shows",
        unit: "series",
    },
    SectionSpec {
        label: "Anime",
        emoji: "🍥",
        dir: "anime",
        unit: "series",
    },
];

/// Statistics for one library section.
#[derive(Debug, Default)]
pub struct SectionStats {
    label: &'static str,
    emoji: &'static str,
    unit: &'static str,
    exists: bool,
    titles: usize,
    videos: usize,
    empty: Vec<String>,
}

/// Full inventory across all library sections.
#[derive(Debug, Default)]
pub struct LibraryReport {
    sections: Vec<SectionStats>,
}

impl LibraryReport {
    /// Total titles across every section.
    #[must_use]
    pub fn total_titles(&self) -> usize {
        self.sections.iter().map(|s| s.titles).sum()
    }

    /// Total playable video files across every section.
    #[must_use]
    pub fn total_videos(&self) -> usize {
        self.sections.iter().map(|s| s.videos).sum()
    }

    /// Folder paths that contain no video file.
    #[must_use]
    pub fn empty_folders(&self) -> Vec<&str> {
        self.sections
            .iter()
            .flat_map(|s| s.empty.iter().map(String::as_str))
            .collect()
    }

    /// Renders the report as Telegram HTML.
    #[must_use]
    pub fn render_html(&self) -> String {
        let mut lines = vec![
            "🌊 <b>領域 RYOIKI • Media Library</b>".to_string(),
            "━━━━━━━━━━━━━━━━━━━━━━━".to_string(),
        ];
        for section in &self.sections {
            lines.push(section.render_line());
        }
        lines.push(format!(
            "📦 <b>Total:</b> {} titles · {} video files",
            self.total_titles(),
            self.total_videos()
        ));
        append_empty_folders(&mut lines, &self.empty_folders());
        lines.push("━━━━━━━━━━━━━━━━━━━━━━━".to_string());
        lines.join("\n")
    }
}

impl SectionStats {
    fn render_line(&self) -> String {
        if !self.exists {
            return format!("{} <b>{}</b> — not found", self.emoji, self.label);
        }
        let unit = if self.titles == 1 {
            self.unit
        } else {
            match self.unit {
                "series" => "series",
                _ => "titles",
            }
        };
        format!(
            "{} <b>{}</b> — {} {unit} · {}",
            self.emoji,
            self.label,
            self.titles,
            plural_videos(self.videos)
        )
    }
}

fn plural_videos(count: usize) -> String {
    if count == 1 {
        "1 video".to_string()
    } else {
        format!("{count} videos")
    }
}

fn append_empty_folders(lines: &mut Vec<String>, empty: &[&str]) {
    if empty.is_empty() {
        return;
    }
    lines.push(String::new());
    lines.push(format!("⚠️ <b>No video file ({}):</b>", empty.len()));
    for path in empty.iter().take(10) {
        lines.push(format!("• <code>{}</code>", escape_html(path)));
    }
    if empty.len() > 10 {
        lines.push(format!("<i>…and {} more</i>", empty.len() - 10));
    }
}

/// Scans `base/movies`, `base/shows`, and `base/anime`.
#[must_use]
pub fn scan_library(base: &Path) -> LibraryReport {
    let sections = SECTIONS
        .iter()
        .map(|spec| scan_section(base, spec))
        .collect();
    LibraryReport { sections }
}

fn scan_section(base: &Path, spec: &SectionSpec) -> SectionStats {
    let root = base.join(spec.dir);
    let mut stats = SectionStats {
        label: spec.label,
        emoji: spec.emoji,
        unit: spec.unit,
        exists: root.is_dir(),
        ..SectionStats::default()
    };
    if !stats.exists {
        return stats;
    }

    for title_dir in sorted_dirs(&root) {
        stats.titles += 1;
        let files = count_videos(&title_dir, 0);
        if files == 0 {
            let name = title_dir
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            stats.empty.push(format!("{}/{name}", spec.dir));
        } else {
            stats.videos += files;
        }
    }
    stats.videos += count_loose_videos(&root);
    stats
}

fn count_loose_videos(dir: &Path) -> usize {
    fs::read_dir(dir).map_or(0, |entries| {
        entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && is_video_file(path))
            .count()
    })
}

fn sorted_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    dirs
}

fn count_videos(dir: &Path, depth: usize) -> usize {
    if depth > MAX_DEPTH {
        return 0;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                count_videos(&path, depth + 1)
            } else {
                usize::from(is_video_file(&path))
            }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_base(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ryoiki_library_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn test_scan_counts_and_flags_empty() {
        let base = temp_base("scan");
        let movie = base.join("movies/Epic (2026)");
        let _ = fs::create_dir_all(&movie);
        let _ = fs::write(movie.join("Epic.mkv"), b"x");

        let broken = base.join("movies/Ghost (2024)");
        let _ = fs::create_dir_all(&broken);
        let _ = fs::write(broken.join("readme.txt"), b"x");

        let season = base.join("shows/Some Show/Season 01");
        let _ = fs::create_dir_all(&season);
        let _ = fs::write(season.join("ep1.mp4"), b"x");

        let report = scan_library(&base);
        let movies = &report.sections[0];
        assert_eq!(movies.titles, 2);
        assert_eq!(movies.videos, 1);
        assert_eq!(movies.empty.len(), 1);
        assert!(movies.empty[0].contains("Ghost (2024)"));

        let shows = &report.sections[1];
        assert_eq!(shows.titles, 1);
        assert_eq!(shows.videos, 1);

        assert!(!report.sections[2].exists);
        assert_eq!(report.total_titles(), 3);
        assert_eq!(report.total_videos(), 2);

        let html = report.render_html();
        assert!(html.contains("Media Library"));
        assert!(html.contains("Ghost (2024)"));
        assert!(html.contains("not found"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    #[ignore = "reads the real Jellyfin library on this machine"]
    fn smoke_real_library() {
        let base =
            std::path::Path::new(&std::env::var("HOME").unwrap_or_default()).join("jellyfin/media");
        let report = scan_library(&base);
        println!("SMOKE_TITLES={}", report.total_titles());
        println!("SMOKE_VIDEOS={}", report.total_videos());
        println!("SMOKE_EMPTY={}", report.empty_folders().len());
        println!("{}", report.render_html());
    }

    #[test]
    fn test_missing_base_is_graceful() {
        let base = temp_base("missing");
        let report = scan_library(&base);
        assert!(report.sections.iter().all(|s| !s.exists));
        assert_eq!(report.empty_folders(), Vec::<&str>::new());
        assert!(report.render_html().contains("Media Library"));
    }
}
