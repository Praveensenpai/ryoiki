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
    width: Option<u32>,
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
            "stream=codec_type,width,height:stream_tags=language",
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
            probe.resolution = match (s.width, s.height) {
                (Some(width), Some(height)) => resolution_for_dimensions(width, height),
                (_, Some(height)) => canonical_resolution_for_height(height),
                _ => None,
            };
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

fn resolution_for_dimensions(width: u32, height: u32) -> Option<String> {
    canonical_resolution_for_height(effective_height(width, height))
}

/// Projects a frame to its equivalent 16:9 height.
///
/// Cinematic masters are often stored cropped (`1920x800`, 2.40:1). Height alone
/// would tag that 720p even though the encode is 1080p-class; the width carries
/// the resolution. Project the longer side to 16:9 and keep the taller result so
/// cropped and letterboxed frames keep their true label.
fn effective_height(width: u32, height: u32) -> u32 {
    let long = width.max(height);
    let short = width.min(height);
    let projected = u32::try_from(u64::from(long) * 9 / 16).unwrap_or(short);
    short.max(projected)
}

/// Canonical marketing label for a vertical pixel height.
///
/// Kept in sync with the thresholds used by [`resolution_for_dimensions`] so a
/// probed frame and a label parsed from a filename collapse to the same tag.
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
        .map(|(width, height)| effective_height(width, height))
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
mod tests;
