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

fn is_tracker_prefix(prefix: &str) -> bool {
    prefix.contains('.')
        || [
            "tamilmv",
            "tamilblasters",
            "tamilrockers",
            "cinevood",
            "moviesmod",
            "vegamovies",
            "bolly4u",
            "extramovies",
            "worldfree4u",
            "1tamil",
        ]
        .iter()
        .any(|p| prefix.contains(p))
}

fn strip_tracker_prefixes(input: &str) -> String {
    let mut s = input.trim();
    if let Some(pos) = s.find(" - ") {
        let prefix = s[..pos].to_ascii_lowercase();
        if is_tracker_prefix(&prefix) {
            s = &s[pos + 3..];
        }
    } else if let Some(pos) = s.find(" — ") {
        let prefix = s[..pos].to_ascii_lowercase();
        if is_tracker_prefix(&prefix) {
            s = &s[pos + 4..];
        }
    }

    let mut trimmed = s.trim();
    while trimmed.starts_with('[') {
        if let Some(end) = trimmed.find(']') {
            let b = &trimmed[1..end].to_ascii_lowercase();
            if !["1080p", "720p", "2160p", "4k", "sp", "remux"]
                .iter()
                .any(|q| b.contains(q))
            {
                trimmed = trimmed[end + 1..].trim();
                continue;
            }
        }
        break;
    }
    trimmed.to_string()
}

pub(crate) fn extract_resolution(input: &str) -> Option<String> {
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
    let tokens: Vec<&str> = input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect();

    let mut found = Vec::new();
    for token in tokens {
        let lower = token.to_ascii_lowercase();
        if lower == "multi" {
            return Some("Multi".to_string());
        }
        for &lang in super::probe::KNOWN_LANGUAGES {
            if lower == lang.to_ascii_lowercase() && !found.contains(&lang) {
                found.push(lang);
            }
        }
    }

    match found.len().cmp(&1) {
        std::cmp::Ordering::Equal => Some(found[0].to_string()),
        std::cmp::Ordering::Greater => {
            let non_english: Vec<&&str> = found.iter().filter(|&&l| l != "English").collect();
            if non_english.len() == 1 {
                Some((*non_english[0]).to_string())
            } else {
                Some("Multi".to_string())
            }
        }
        std::cmp::Ordering::Less => None,
    }
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
mod tests;
