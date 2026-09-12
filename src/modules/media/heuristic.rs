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
    let (season, episode) = extract_season_episode(&clean_stem);
    let year = extract_year(&clean_stem);

    let media_type = if season.is_some() || episode.is_some() {
        MediaType::Show
    } else {
        MediaType::Movie
    };

    let title = extract_title(&clean_stem, year, season, episode);
    let clean_name = format_clean_name(&title, year, season, episode, resolution.as_deref(), ext);

    MediaInfo {
        media_type,
        title,
        year,
        season,
        episode,
        resolution,
        clean_name,
        engine: ClassificationEngine::Heuristic,
    }
}

fn strip_tracker_prefixes(input: &str) -> String {
    let mut s = input.trim();
    if let Some(pos) = s.find(" - ") {
        let prefix = &s[..pos];
        if prefix.to_ascii_lowercase().contains("www.")
            || prefix.to_ascii_lowercase().contains(".com")
            || prefix.to_ascii_lowercase().contains(".org")
            || prefix.to_ascii_lowercase().contains(".center")
            || prefix.to_ascii_lowercase().contains(".cc")
        {
            s = &s[pos + 3..];
        }
    }

    let cleaned = s.trim_start_matches('[');
    if let Some(end_bracket) = cleaned.find(']') {
        let bracket_content = &cleaned[..end_bracket].to_ascii_lowercase();
        if bracket_content.contains("tgx")
            || bracket_content.contains("yts")
            || bracket_content.contains("eztv")
            || bracket_content.contains("psa")
        {
            return cleaned[end_bracket + 1..].trim().to_string();
        }
    }
    s.to_string()
}

fn extract_resolution(input: &str) -> Option<String> {
    let lower = input.to_ascii_lowercase();
    for res in ["2160p", "4k", "1080p", "720p", "480p"] {
        if lower.contains(res) {
            return Some(if res == "4k" {
                "2160p".to_string()
            } else {
                res.to_string()
            });
        }
    }
    None
}

fn extract_season_episode(input: &str) -> (Option<u32>, Option<u32>) {
    let chars: Vec<char> = input.chars().collect();
    for i in 0..chars.len() {
        if (chars[i] == 's' || chars[i] == 'S')
            && i + 1 < chars.len()
            && chars[i + 1].is_ascii_digit()
        {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            let s_str: String = chars[i + 1..j].iter().collect();
            if j < chars.len() && (chars[j] == 'e' || chars[j] == 'E') && j + 1 < chars.len() {
                let mut k = j + 1;
                while k < chars.len() && chars[k].is_ascii_digit() {
                    k += 1;
                }
                let e_str: String = chars[j + 1..k].iter().collect();
                if let (Ok(s), Ok(e)) = (s_str.parse::<u32>(), e_str.parse::<u32>()) {
                    return (Some(s), Some(e));
                }
            }
        }
    }
    (None, None)
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

fn extract_title(
    input: &str,
    year: Option<u32>,
    season: Option<u32>,
    _episode: Option<u32>,
) -> String {
    let mut cut_idx = input.len();

    if let Some(yr) = year {
        let yr_str = yr.to_string();
        if let Some(pos) = input.find(&yr_str) {
            cut_idx = cut_idx.min(pos);
        }
    }

    if season.is_some() {
        for marker in ["S0", "s0", "S1", "s1", "S2", "s2", "Season", "season"] {
            if let Some(pos) = input.find(marker) {
                cut_idx = cut_idx.min(pos);
            }
        }
    }

    for tag in [
        "1080p",
        "720p",
        "2160p",
        "4k",
        "WEB-DL",
        "WEBRip",
        "BluRay",
        "HDR",
        "HDTV",
        "x264",
        "x265",
        "HEVC",
        "Kannada",
        "Tamil",
        "Hindi",
        "Telugu",
        "Malayalam",
        "English",
    ] {
        let lower = input.to_ascii_lowercase();
        if let Some(pos) = lower.find(&tag.to_ascii_lowercase()) {
            cut_idx = cut_idx.min(pos);
        }
    }

    let mut title = input[..cut_idx]
        .replace(['.', '_', '-'], " ")
        .replace(['(', ')', '[', ']'], "");

    title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() {
        input.to_string()
    } else {
        title
    }
}

fn format_clean_name(
    title: &str,
    year: Option<u32>,
    season: Option<u32>,
    episode: Option<u32>,
    res: Option<&str>,
    ext: &str,
) -> String {
    let res_tag = res.map_or_else(String::new, |r| format!(" [{r}]"));
    if let (Some(s), Some(e)) = (season, episode) {
        format!("{title} - S{s:02}E{e:02}{res_tag}.{ext}")
    } else if let Some(yr) = year {
        format!("{title} ({yr}){res_tag}.{ext}")
    } else {
        format!("{title}{res_tag}.{ext}")
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
        assert_eq!(info.resolution.as_deref(), Some("1080p"));
        assert_eq!(info.clean_name, "Love Mocktail 3 (2026) [1080p].mkv");
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
}
