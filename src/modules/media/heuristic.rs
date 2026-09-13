use super::{ClassificationEngine, MediaInfo, MediaType};
use std::path::Path;

pub fn classify_media_heuristic(raw_name: &str) -> MediaInfo {
    let path = Path::new(raw_name);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(raw_name);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("mkv");

    let clean_stem = strip_tracker_prefixes(stem);
    let resolution = extract_resolution(&clean_stem);
    let is_extra = is_extra_content(raw_name);
    let (season, episode) = extract_season_episode(&clean_stem);
    let year = extract_year(&clean_stem);
    let language = extract_language(&clean_stem);

    let is_anime = is_anime_marker(raw_name, language.as_deref());
    let media_type = if is_anime {
        MediaType::Anime
    } else if season.is_some() || episode.is_some() {
        MediaType::Show
    } else {
        MediaType::Movie
    };

    let title = extract_title(&clean_stem, year, season, episode);
    let extra_desc = if is_extra {
        extract_extra_desc(&clean_stem)
    } else {
        None
    };
    let clean_name = format_clean_name(&CleanNameOptions {
        title: &title,
        year,
        season,
        episode,
        is_extra,
        extra_desc: extra_desc.as_deref(),
        language: language.as_deref(),
        resolution: resolution.as_deref(),
        ext,
    });

    MediaInfo {
        media_type,
        title,
        year,
        season,
        episode,
        resolution,
        language,
        clean_name,
        is_extra,
        engine: ClassificationEngine::Heuristic,
    }
}

const ANIME_MARKERS: &str =
    "moozzi2 subsplease erai-raws horriblesubs judas asw ani ember b-global anime [sp";

fn is_anime_marker(input: &str, language: Option<&str>) -> bool {
    language.is_some_and(|l| l.eq_ignore_ascii_case("Japanese"))
        || ANIME_MARKERS
            .split_whitespace()
            .any(|m| input.to_ascii_lowercase().contains(m))
}

fn is_extra_content(input: &str) -> bool {
    let lower = input.to_ascii_lowercase();
    lower.contains("[sp")
        || lower.contains("ncop")
        || lower.contains("nced")
        || lower.contains("menu")
        || lower.contains("extra")
        || lower.contains("pv")
}

fn extract_extra_desc(input: &str) -> Option<String> {
    let lower = input.to_ascii_lowercase();
    let start_idx = lower.find("[sp").or_else(|| lower.find("sp"))?;
    let mut tail = &input[start_idx..];
    for cut in [" (", " 1080p", " 720p", " 2160p", " 4k", ".mkv"] {
        if let Some(pos) = tail.to_ascii_lowercase().find(cut) {
            tail = &tail[..pos];
        }
    }
    let trimmed = tail.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn strip_tracker_prefixes(input: &str) -> String {
    let mut s = input.trim();
    if let Some(pos) = s.find(" - ") {
        let prefix = s[..pos].to_ascii_lowercase();
        if [".com", ".org", ".center", ".cc", "www."]
            .iter()
            .any(|d| prefix.contains(d))
        {
            s = &s[pos + 3..];
        }
    }

    let trimmed = s.trim();
    if trimmed.starts_with('[') {
        if let Some(end) = trimmed.find(']') {
            let b = &trimmed[1..end].to_ascii_lowercase();
            if !["1080p", "720p", "2160p", "4k", "sp"]
                .iter()
                .any(|q| b.contains(q))
            {
                return trimmed[end + 1..].trim().to_string();
            }
        }
    }
    s.to_string()
}

fn extract_resolution(input: &str) -> Option<String> {
    let lower = input.to_ascii_lowercase();
    if lower.contains("2160p") || lower.contains("4k") || lower.contains("3840x2160") {
        Some("2160p".to_string())
    } else if lower.contains("1080p") || lower.contains("1920x1080") || lower.contains("1080i") {
        Some("1080p".to_string())
    } else if lower.contains("720p") || lower.contains("1280x720") {
        Some("720p".to_string())
    } else if lower.contains("480p") {
        Some("480p".to_string())
    } else {
        None
    }
}

fn extract_season_episode(input: &str) -> (Option<u32>, Option<u32>) {
    let chars: Vec<char> = input.chars().collect();
    let mut detected_season = None;
    for i in 0..chars.len() {
        if matches!(chars[i], 's' | 'S') && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if let Ok(s) = chars[i + 1..j].iter().collect::<String>().parse::<u32>() {
                detected_season = Some(s);
            }
            let parse_ep = |start: usize| -> Option<u32> {
                let mut k = start;
                while k < chars.len() && chars[k].is_ascii_digit() {
                    k += 1;
                }
                chars[start..k]
                    .iter()
                    .collect::<String>()
                    .parse::<u32>()
                    .ok()
            };
            if j < chars.len() && matches!(chars[j], 'e' | 'E') {
                if let (Some(s), Some(e)) = (detected_season, parse_ep(j + 1)) {
                    return (Some(s), Some(e));
                }
            }
            let mut k = j;
            while k < chars.len() && matches!(chars[k], ' ' | '-' | '.') {
                k += 1;
            }
            if k < chars.len() && chars[k].is_ascii_digit() {
                if let (Some(s), Some(e)) = (detected_season, parse_ep(k)) {
                    return (Some(s), Some(e));
                }
            }
        }
    }
    let lower = input.to_ascii_lowercase();
    if let Some(sp_idx) = lower.find("sp") {
        let digits: String = lower[sp_idx + 2..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(ep) = digits.parse::<u32>() {
            return (detected_season, Some(ep));
        }
    }
    (detected_season, None)
}

fn extract_year(input: &str) -> Option<u32> {
    let words: Vec<&str> = input.split(|c: char| !c.is_ascii_digit()).collect();
    for w in words.into_iter().rev() {
        if w.len() == 4 {
            if let Ok(yr) = w.parse::<u32>() {
                if (1920..=2099).contains(&yr) {
                    return Some(yr);
                }
            }
        }
    }
    None
}

const TITLE_CUT_TAGS: &[&str] = &[
    "1080p",
    "720p",
    "2160p",
    "4k",
    "web-dl",
    "webrip",
    "bluray",
    "hdr",
    "hdtv",
    "x264",
    "x265",
    "hevc",
    "kannada",
    "tamil",
    "hindi",
    "telugu",
    "malayalam",
    "english",
    "japanese",
];

const KNOWN_LANGUAGES: &[&str] = &[
    "Malayalam",
    "Tamil",
    "Telugu",
    "Kannada",
    "Hindi",
    "English",
    "Korean",
    "Japanese",
    "Spanish",
    "French",
];

fn extract_title(
    input: &str,
    year: Option<u32>,
    season: Option<u32>,
    _episode: Option<u32>,
) -> String {
    let mut cut_idx = input.len();
    let lower = input.to_ascii_lowercase();

    if let Some(yr) = year {
        if let Some(pos) = input.find(&yr.to_string()) {
            cut_idx = cut_idx.min(pos);
        }
    }

    if season.is_some() {
        for s in 0..=9 {
            for m in [format!("s{s}"), format!("season {s}")] {
                if let Some(pos) = lower.find(&m) {
                    cut_idx = cut_idx.min(pos);
                }
            }
        }
        for marker in ["season", "sp"] {
            if let Some(pos) = lower.find(marker) {
                cut_idx = cut_idx.min(pos);
            }
        }
    }

    for tag in TITLE_CUT_TAGS {
        if let Some(pos) = lower.find(tag) {
            cut_idx = cut_idx.min(pos);
        }
    }

    let title = input[..cut_idx]
        .replace(['.', '_', '-'], " ")
        .replace(['(', ')', '[', ']'], "");
    let words = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if words.is_empty() {
        input.to_string()
    } else {
        words
    }
}

fn extract_language(input: &str) -> Option<String> {
    let lower = input.to_ascii_lowercase();
    KNOWN_LANGUAGES
        .iter()
        .find(|l| lower.contains(&l.to_ascii_lowercase()))
        .map(|l| (*l).to_string())
}

struct CleanNameOptions<'a> {
    title: &'a str,
    year: Option<u32>,
    season: Option<u32>,
    episode: Option<u32>,
    is_extra: bool,
    extra_desc: Option<&'a str>,
    language: Option<&'a str>,
    resolution: Option<&'a str>,
    ext: &'a str,
}

fn format_clean_name(opts: &CleanNameOptions<'_>) -> String {
    let lang = opts
        .language
        .map_or_else(String::new, |l| format!(" [{l}]"));
    let res = opts
        .resolution
        .map_or_else(String::new, |r| format!(" [{r}]"));
    let ext = opts.ext;

    if opts.is_extra {
        let desc = opts.extra_desc.unwrap_or("Special");
        opts.season.map_or_else(
            || format!("{} - {desc}{lang}{res}.{ext}", opts.title),
            |s| format!("{} - S{s:02} {desc}{lang}{res}.{ext}", opts.title),
        )
    } else if let (Some(s), Some(e)) = (opts.season, opts.episode) {
        format!("{} - S{s:02}E{e:02}{lang}{res}.{ext}", opts.title)
    } else if let Some(yr) = opts.year {
        format!("{} ({yr}){lang}{res}.{ext}", opts.title)
    } else {
        format!("{}{lang}{res}.{ext}", opts.title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic_movie_cleaning() {
        let raw = "www.1TamilMV.center - Love Mocktail 3 (2026) Kannada TRUE WEB-DL - 1080p - AVC - (DD_5.1 - 192Kbps _ AAC 2.0) - 2GB - ESub.mkv";
        let info = classify_media_heuristic(raw);
        assert_eq!(info.media_type, MediaType::Movie);
        assert_eq!(info.title, "Love Mocktail 3");
        assert_eq!(info.year, Some(2026));
        assert_eq!(info.language.as_deref(), Some("Kannada"));
        assert_eq!(info.resolution.as_deref(), Some("1080p"));
        assert_eq!(
            info.clean_name,
            "Love Mocktail 3 (2026) [Kannada] [1080p].mkv"
        );
    }

    #[test]
    fn test_heuristic_show_cleaning() {
        let raw = "House.of.the.Dragon.S02E04.1080p.WEB.H264-SUCCESS.mkv";
        let info = classify_media_heuristic(raw);
        assert_eq!(info.media_type, MediaType::Show);
        assert_eq!(info.title, "House of the Dragon");
        assert_eq!(info.season, Some(2));
        assert_eq!(info.episode, Some(4));
        assert_eq!(info.clean_name, "House of the Dragon - S02E04 [1080p].mkv");
    }

    #[test]
    fn test_heuristic_anime_cleaning() {
        let raw = "[Moozzi2] Yuru Camp S3 - 01 (BD 1920x1080 x265-10Bit Flac).mkv";
        let info = classify_media_heuristic(raw);
        assert_eq!(info.media_type, MediaType::Anime);
        assert_eq!(info.title, "Yuru Camp");
        assert_eq!(info.season, Some(3));
        assert_eq!(info.episode, Some(1));
        assert_eq!(info.resolution.as_deref(), Some("1080p"));
        assert_eq!(info.clean_name, "Yuru Camp - S03E01 [1080p].mkv");
        assert!(!info.is_extra);
    }

    #[test]
    fn test_heuristic_anime_specials() {
        let s2_raw = "[Moozzi2] Yuru Camp S2 [SP01] NCOP (BD 1920x1080 x265-10Bit Flac).mkv";
        let info2 = classify_media_heuristic(s2_raw);
        assert_eq!(info2.title, "Yuru Camp");
        assert_eq!(info2.season, Some(2));
        assert!(info2.is_extra);
        assert_eq!(info2.clean_name, "Yuru Camp - S02 [SP01] NCOP [1080p].mkv");

        let s3_raw = "[Moozzi2] Yuru Camp S3 [SP01] NCED (BD 1920x1080 x265-10Bit Flac).mkv";
        let info3 = classify_media_heuristic(s3_raw);
        assert_eq!(info3.title, "Yuru Camp");
        assert_eq!(info3.season, Some(3));
        assert!(info3.is_extra);
        assert_eq!(info3.clean_name, "Yuru Camp - S03 [SP01] NCED [1080p].mkv");

        let menu = "[Moozzi2] Yuru Camp S3 [SP00] Menu - 01 (BD 1920x1080 x265-10Bit Flac).mkv";
        let info_menu = classify_media_heuristic(menu);
        assert_eq!(
            info_menu.clean_name,
            "Yuru Camp - S03 [SP00] Menu - 01 [1080p].mkv"
        );
    }
}
