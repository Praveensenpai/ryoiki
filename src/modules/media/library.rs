//! Read-only inventory of the Jellyfin media library.
//!
//! Counts titles and playable video files per section, names every title, and
//! reports the audio languages of the first video in each title folder. Titles
//! are paginated because a full library overflows a single Telegram message.

use std::fs;
use std::path::{Path, PathBuf};

use super::organizer::pathing::is_video_file;
use crate::notify::client::escape_html;

/// Maximum directory depth searched inside a single title folder.
const MAX_DEPTH: usize = 4;

/// Characters allowed per page body, leaving room for the timestamp footer.
const PAGE_BUDGET: usize = 3200;

const CONTINUED_HEADER: &str = "🌊 <b>領域 RYOIKI • Media Library</b> <i>(cont.)</i>";

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

/// One title folder with its file count and audio languages.
#[derive(Debug, Clone)]
pub struct TitleEntry {
    name: String,
    videos: usize,
    languages: Vec<String>,
}

impl TitleEntry {
    fn new(name: String, videos: usize, languages: Vec<String>) -> Self {
        Self {
            name,
            videos,
            languages,
        }
    }

    /// Renders the title as one list line.
    #[must_use]
    pub fn render_line(&self) -> String {
        let languages = if self.languages.is_empty() {
            "audio ?".to_string()
        } else {
            self.languages.join(", ")
        };
        format!(
            "  • {} — {} · {}",
            escape_html(&self.name),
            plural_videos(self.videos),
            escape_html(&languages)
        )
    }
}

/// Statistics for one library section.
#[derive(Debug, Default)]
pub struct SectionStats {
    label: &'static str,
    emoji: &'static str,
    unit: &'static str,
    exists: bool,
    titles: Vec<TitleEntry>,
    empty: Vec<String>,
}

impl SectionStats {
    fn video_count(&self) -> usize {
        self.titles.iter().map(|t| t.videos).sum()
    }

    fn render_header(&self) -> String {
        if !self.exists {
            return format!("{} <b>{}</b> — not found", self.emoji, self.label);
        }
        let unit = if self.titles.len() == 1 {
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
            self.titles.len(),
            plural_videos(self.video_count())
        )
    }
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
        self.sections.iter().map(|s| s.titles.len()).sum()
    }

    /// Total playable video files across every section.
    #[must_use]
    pub fn total_videos(&self) -> usize {
        self.sections.iter().map(SectionStats::video_count).sum()
    }

    /// Folder paths that contain no video file.
    #[must_use]
    pub fn empty_folders(&self) -> Vec<&str> {
        self.sections
            .iter()
            .flat_map(|s| s.empty.iter().map(String::as_str))
            .collect()
    }

    /// Renders the report split into Telegram-sized messages.
    #[must_use]
    pub fn render_pages(&self) -> Vec<String> {
        paginate(&self.build_body_lines())
    }

    fn build_body_lines(&self) -> Vec<String> {
        let mut lines = vec![
            "🌊 <b>領域 RYOIKI • Media Library</b>".to_string(),
            "━━━━━━━━━━━━━━━━━━━━━━━".to_string(),
        ];
        for section in &self.sections {
            lines.push(section.render_header());
            for title in &section.titles {
                lines.push(title.render_line());
            }
        }
        lines.push(format!(
            "📦 <b>Total:</b> {} titles · {} video files",
            self.total_titles(),
            self.total_videos()
        ));
        append_empty_folders(&mut lines, &self.empty_folders());
        lines.push("━━━━━━━━━━━━━━━━━━━━━━━".to_string());
        lines
    }
}

fn append_empty_folders(lines: &mut Vec<String>, empty: &[&str]) {
    if empty.is_empty() {
        return;
    }
    lines.push(String::new());
    lines.push(format!("⚠️ <b>No video file ({}):</b>", empty.len()));
    for path in empty {
        lines.push(format!("• <code>{}</code>", escape_html(path)));
    }
}

fn plural_videos(count: usize) -> String {
    if count == 1 {
        "1 file".to_string()
    } else {
        format!("{count} files")
    }
}

/// Splits body lines into pages that each fit a Telegram message.
fn paginate(lines: &[String]) -> Vec<String> {
    let Some((header, rest)) = lines.split_first() else {
        return Vec::new();
    };

    let mut pages = Vec::new();
    let mut current = header.clone();
    let mut used = current.chars().count();

    for line in rest {
        let cost = line.chars().count() + 1;
        if used + cost > PAGE_BUDGET && used > header.chars().count() {
            pages.push(current);
            current = CONTINUED_HEADER.to_string();
            used = current.chars().count();
        }
        current.push('\n');
        current.push_str(line);
        used += cost;
    }

    pages.push(current);
    pages
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
        let name = title_dir
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let files = collect_videos(&title_dir, 0);
        if files.is_empty() {
            stats.empty.push(format!("{}/{name}", spec.dir));
            continue;
        }
        let languages = languages_from_files(&files);
        stats
            .titles
            .push(TitleEntry::new(name, files.len(), languages));
    }
    stats
}

/// Collects the distinct `[Language]` tags from video filenames.
///
/// The organizer already writes the language into every filename
/// (`Epic (2026) [Tamil] [1080p].mkv`), so no media probing is needed.
fn languages_from_files(files: &[PathBuf]) -> Vec<String> {
    let mut languages: Vec<String> = Vec::new();
    for path in files {
        let Some(stem) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        for tag in bracket_tags(stem) {
            if is_language_tag(&tag) && !languages.iter().any(|l| l.eq_ignore_ascii_case(&tag)) {
                languages.push(tag);
            }
        }
    }
    languages
}

/// Extracts the contents of every `[...]` group in a filename.
fn bracket_tags(text: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut current: Option<String> = None;
    for ch in text.chars() {
        match (ch, current.as_mut()) {
            ('[', None) => current = Some(String::new()),
            (']', Some(buf)) => {
                tags.push(buf.trim().to_string());
                current = None;
            }
            (c, Some(buf)) => buf.push(c),
            (_, None) => {}
        }
    }
    tags
}

/// Known audio language names, matched case-insensitively.
const LANGUAGE_NAMES: [&str; 34] = [
    "Tamil",
    "Telugu",
    "Hindi",
    "Malayalam",
    "Kannada",
    "English",
    "Japanese",
    "Korean",
    "Chinese",
    "Mandarin",
    "Cantonese",
    "Spanish",
    "French",
    "German",
    "Italian",
    "Portuguese",
    "Russian",
    "Arabic",
    "Turkish",
    "Thai",
    "Vietnamese",
    "Indonesian",
    "Bengali",
    "Marathi",
    "Gujarati",
    "Punjabi",
    "Odia",
    "Urdu",
    "Nepali",
    "Sinhala",
    "Dutch",
    "Polish",
    "Swedish",
    "Danish",
];

/// Whether a bracketed filename tag names an audio language.
fn is_language_tag(tag: &str) -> bool {
    LANGUAGE_NAMES
        .iter()
        .any(|language| language.eq_ignore_ascii_case(tag))
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

fn collect_videos(dir: &Path, depth: usize) -> Vec<PathBuf> {
    if depth > MAX_DEPTH {
        return Vec::new();
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_videos(&path, depth + 1));
        } else if is_video_file(&path) {
            files.push(path);
        }
    }
    files.sort();
    files
}

#[cfg(test)]
mod tests;
