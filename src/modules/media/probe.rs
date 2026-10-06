use serde::Deserialize;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaProbe {
    pub duration_mins: Option<u64>,
    pub audio_languages: Vec<String>,
    pub audio_stream_count: usize,
    pub primary_language: Option<String>,
    pub resolution: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct FfprobeOutput {
    streams: Option<Vec<FfprobeStream>>,
    format: Option<FfprobeFormat>,
}

#[derive(Debug, Deserialize, Default)]
struct FfprobeStream {
    codec_type: Option<String>,
    height: Option<u32>,
    tags: Option<FfprobeTags>,
}

#[derive(Debug, Deserialize, Default)]
struct FfprobeTags {
    language: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct FfprobeFormat {
    duration: Option<String>,
}

/// Probes a media file via `ffprobe` to extract duration, audio languages, and resolution.
#[must_use]
pub fn probe_media_file(path: &Path) -> Option<MediaProbe> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-show_entries",
            "stream=codec_type,height:stream_tags=language",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_ffprobe_json(&output.stdout)
}

#[must_use]
pub fn parse_ffprobe_json(raw_json: &[u8]) -> Option<MediaProbe> {
    let parsed: FfprobeOutput = serde_json::from_slice(raw_json).ok()?;
    let mut probe = MediaProbe::default();

    if let Some(fmt) = parsed.format {
        probe.duration_mins = parse_duration_mins(fmt.duration.as_deref());
    }

    if let Some(streams) = parsed.streams {
        populate_streams_info(streams, &mut probe);
    }

    probe.primary_language = resolve_primary_language(&probe.audio_languages);

    Some(probe)
}

fn parse_duration_mins(dur_str: Option<&str>) -> Option<u64> {
    let s = dur_str?;
    let int_part = s.split('.').next()?;
    let total_secs: u64 = int_part.parse().ok()?;
    if total_secs == 0 {
        return None;
    }
    let rem = total_secs % 60;
    let mins = total_secs / 60;
    if rem >= 30 {
        Some(mins + 1)
    } else {
        Some(mins)
    }
}

fn populate_streams_info(streams: Vec<FfprobeStream>, probe: &mut MediaProbe) {
    for s in streams {
        let codec = s.codec_type.as_deref().unwrap_or_default();
        if codec == "video" && probe.resolution.is_none() {
            probe.resolution = s.height.and_then(height_to_resolution);
        } else if codec == "audio" {
            probe.audio_stream_count += 1;
            extract_audio_language(s.tags.as_ref(), &mut probe.audio_languages);
        }
    }
}

fn extract_audio_language(tags: Option<&FfprobeTags>, list: &mut Vec<String>) {
    let Some(lang) = tags.and_then(|t| t.language.as_deref()) else {
        return;
    };

    let trimmed = lang.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("und") {
        return;
    }

    let mapped = map_language_code(trimmed);
    if !list.contains(&mapped) {
        list.push(mapped);
    }
}

fn height_to_resolution(height: u32) -> Option<String> {
    canonical_resolution_for_height(height)
}

/// Canonical marketing label for a vertical pixel height.
///
/// Kept in sync with the thresholds used by [`height_to_resolution`] so a probed
/// frame and a label parsed from a filename collapse to the same tag.
fn canonical_resolution_for_height(height: u32) -> Option<String> {
    if height >= 2000 {
        Some("2160p".to_string())
    } else if height >= 1000 {
        Some("1080p".to_string())
    } else if height >= 700 {
        Some("720p".to_string())
    } else if height >= 450 {
        Some("480p".to_string())
    } else {
        None
    }
}

/// Normalizes an arbitrary resolution label to a single canonical form.
///
/// The classifier returns raw dimensions (`1024x768`) and marketing labels
/// (`768p`) interchangeably for the same encode. Both collapse to one tag so
/// filenames stay consistent across every file in a release.
#[must_use]
pub fn normalize_resolution(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let lower = trimmed.to_ascii_lowercase();

    let height = split_dimension(&lower)
        .map(|(width, height)| width.min(height))
        .or_else(|| parse_p_height(&lower));

    height.map_or_else(
        || trimmed.to_string(),
        |h| canonical_resolution_for_height(h).unwrap_or_else(|| format!("{h}p")),
    )
}

/// True when `value` looks like a resolution label (`768p`, `1024x768`).
#[must_use]
pub fn is_resolution_label(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    split_dimension(&lower).is_some() || parse_p_height(&lower).is_some()
}

/// Splits `1920x1080` / `1024×768` into `(width, height)`.
fn split_dimension(value: &str) -> Option<(u32, u32)> {
    let normalized = value.replace('×', "x");
    let (width, height) = normalized.split_once('x')?;
    let width = width.trim().parse::<u32>().ok()?;
    let height = height.trim().parse::<u32>().ok()?;
    if width > 0 && height > 0 {
        Some((width, height))
    } else {
        None
    }
}

/// Parses the height out of a `768p` / `1080p` label.
fn parse_p_height(value: &str) -> Option<u32> {
    value
        .strip_suffix('p')
        .and_then(|digits| digits.parse::<u32>().ok())
}

pub const KNOWN_LANGUAGES: &[&str] = &[
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
    "Bengali",
    "Marathi",
    "Punjabi",
    "German",
    "Italian",
    "Chinese",
];

#[must_use]
pub fn map_language_code(code: &str) -> String {
    let lower = code.trim().to_ascii_lowercase();
    match lower.as_str() {
        "mal" | "malayalam" => "Malayalam".to_string(),
        "hin" | "hindi" => "Hindi".to_string(),
        "tam" | "tamil" => "Tamil".to_string(),
        "tel" | "telugu" => "Telugu".to_string(),
        "kan" | "kannada" => "Kannada".to_string(),
        "eng" | "english" => "English".to_string(),
        "kor" | "korean" => "Korean".to_string(),
        "jpn" | "japanese" => "Japanese".to_string(),
        "spa" | "spanish" => "Spanish".to_string(),
        "fra" | "fre" | "french" => "French".to_string(),
        "deu" | "ger" | "german" => "German".to_string(),
        "ita" | "italian" => "Italian".to_string(),
        "zho" | "chi" | "chinese" => "Chinese".to_string(),
        "ben" | "bengali" => "Bengali".to_string(),
        "mar" | "marathi" => "Marathi".to_string(),
        "pan" | "punjabi" => "Punjabi".to_string(),
        other => {
            let mut chars = other.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        }
    }
}

fn resolve_primary_language(languages: &[String]) -> Option<String> {
    if languages.is_empty() {
        return None;
    }
    if languages.len() == 1 {
        return Some(languages[0].clone());
    }

    let non_english: Vec<&String> = languages
        .iter()
        .filter(|l| l.as_str() != "English")
        .collect();

    match non_english.len().cmp(&1) {
        std::cmp::Ordering::Equal => Some(non_english[0].clone()),
        std::cmp::Ordering::Greater => Some("Multi".to_string()),
        std::cmp::Ordering::Less => Some("English".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ffprobe_json() {
        let sample = br#"{
            "streams": [
                {
                    "codec_type": "video",
                    "height": 1080,
                    "tags": { "language": "mal" }
                },
                {
                    "codec_type": "audio",
                    "tags": { "language": "mal" }
                }
            ],
            "format": {
                "duration": "9854.272000"
            }
        }"#;

        let Some(probe) = parse_ffprobe_json(sample) else {
            panic!("Failed to parse sample json");
        };
        assert_eq!(probe.duration_mins, Some(164));
        assert_eq!(probe.resolution.as_deref(), Some("1080p"));
        assert_eq!(probe.audio_languages, vec!["Malayalam"]);
        assert_eq!(probe.primary_language.as_deref(), Some("Malayalam"));
    }

    #[test]
    fn test_resolve_primary_language() {
        assert_eq!(
            resolve_primary_language(&["Malayalam".to_string()]).as_deref(),
            Some("Malayalam")
        );
        assert_eq!(
            resolve_primary_language(&["Malayalam".to_string(), "English".to_string()]).as_deref(),
            Some("Malayalam")
        );
        assert_eq!(
            resolve_primary_language(&["Malayalam".to_string(), "Tamil".to_string()]).as_deref(),
            Some("Multi")
        );
        assert_eq!(
            resolve_primary_language(&["English".to_string()]).as_deref(),
            Some("English")
        );
    }

    #[test]
    fn test_height_to_resolution() {
        assert_eq!(height_to_resolution(2160).as_deref(), Some("2160p"));
        assert_eq!(height_to_resolution(1080).as_deref(), Some("1080p"));
        assert_eq!(height_to_resolution(720).as_deref(), Some("720p"));
        assert_eq!(height_to_resolution(480).as_deref(), Some("480p"));
        assert_eq!(height_to_resolution(360), None);
    }

    #[test]
    fn test_normalize_resolution_collapses_variants() {
        assert_eq!(normalize_resolution("1024x768"), "720p");
        assert_eq!(normalize_resolution("1024×768"), "720p");
        assert_eq!(normalize_resolution("768p"), "720p");
        assert_eq!(normalize_resolution("1920x1080"), "1080p");
        assert_eq!(normalize_resolution("1080p"), "1080p");
        assert_eq!(normalize_resolution("1280x720"), "720p");
        assert_eq!(normalize_resolution("480p"), "480p");
        assert_eq!(normalize_resolution("2160p"), "2160p");
        assert_eq!(normalize_resolution("360p"), "360p");
    }

    #[test]
    fn test_normalize_resolution_ignores_non_resolutions() {
        assert_eq!(normalize_resolution(""), "");
        assert_eq!(normalize_resolution("x264 AAC"), "x264 AAC");
        assert_eq!(normalize_resolution("Japanese"), "Japanese");
    }

    #[test]
    fn test_is_resolution_label() {
        for good in ["768p", "1080p", "1024x768", "1920x1080", "480p"] {
            assert!(is_resolution_label(good), "{good} should be a label");
        }
        for bad in ["x264 AAC", "x265 HEVC", "Japanese", "SP01", ""] {
            assert!(!is_resolution_label(bad), "{bad} should not be a label");
        }
    }
}
