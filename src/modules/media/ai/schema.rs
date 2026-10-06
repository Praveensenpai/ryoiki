use anyhow::{Context, Result};
use serde::Deserialize;
use std::fmt::Write as _;
use std::path::Path;

use super::super::{ClassificationEngine, MediaInfo, MediaType};

#[derive(Debug, Deserialize)]
pub struct AiOutputSchema {
    pub media_type: String,
    pub title: String,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub resolution: Option<String>,
    pub language: Option<String>,
    pub clean_name: Option<String>,
    pub is_extra: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AiBatchItemSchema {
    pub raw_name: String,
    pub media_type: String,
    pub title: String,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub resolution: Option<String>,
    pub language: Option<String>,
    pub clean_name: Option<String>,
    pub is_extra: Option<bool>,
}

pub fn parse_ai_json(json_text: &str, raw_name: &str) -> Result<MediaInfo> {
    let cleaned = super::client::clean_json_text(json_text);
    let schema: AiOutputSchema =
        serde_json::from_str(&cleaned).context("Failed to parse model JSON into schema")?;
    Ok(build_media_info(&schema, raw_name))
}

pub fn parse_batch_ai_json(json_text: &str) -> Result<Vec<(String, MediaInfo)>> {
    let cleaned = super::client::clean_json_text(json_text);
    let items: Vec<AiBatchItemSchema> =
        serde_json::from_str(&cleaned).context("Failed to parse batch JSON array")?;

    let mut results = Vec::with_capacity(items.len());
    for item in items {
        let single_schema = AiOutputSchema {
            media_type: item.media_type,
            title: item.title,
            year: item.year,
            season: item.season,
            episode: item.episode,
            resolution: item.resolution,
            language: item.language,
            clean_name: item.clean_name,
            is_extra: item.is_extra,
        };
        let raw = item.raw_name.clone();
        let info = build_media_info(&single_schema, &raw);
        results.push((raw, info));
    }
    Ok(results)
}

fn build_media_info(schema: &AiOutputSchema, raw_name: &str) -> MediaInfo {
    let media_type = if schema.media_type.eq_ignore_ascii_case("anime") {
        MediaType::Anime
    } else if schema.media_type.eq_ignore_ascii_case("show") {
        MediaType::Show
    } else {
        MediaType::Movie
    };

    let clean_name = resolve_clean_name(schema, media_type, raw_name);
    let lower = raw_name.to_ascii_lowercase();
    let is_extra = schema.is_extra.unwrap_or(false)
        || lower.contains("extra")
        || lower.contains("[sp")
        || lower.contains("ncop")
        || lower.contains("nced");

    MediaInfo {
        media_type,
        title: schema.title.clone(),
        year: schema.year,
        season: schema.season,
        episode: schema.episode,
        resolution: schema.resolution.clone(),
        language: schema.language.clone(),
        clean_name,
        is_extra,
        engine: ClassificationEngine::Ai,
    }
}

fn resolve_clean_name(schema: &AiOutputSchema, media_type: MediaType, raw_name: &str) -> String {
    let ext = Path::new(raw_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mkv");

    let Some(mut cn) = schema.clean_name.clone() else {
        let lang_tag = schema
            .language
            .as_deref()
            .map_or_else(String::new, |l| format!(" [{l}]"));
        let res_tag = schema
            .resolution
            .as_deref()
            .map_or_else(String::new, |r| format!(" [{r}]"));
        return match (media_type, schema.year) {
            (MediaType::Movie, Some(yr)) => {
                format!("{} ({yr}){lang_tag}{res_tag}.{ext}", schema.title)
            }
            _ => format!("{}{lang_tag}{res_tag}.{ext}", schema.title),
        };
    };

    if media_type == MediaType::Movie
        || (media_type == MediaType::Anime && schema.season.is_none() && schema.episode.is_none())
    {
        if let Some(yr) = schema.year {
            let yr_str = format!("({yr})");
            if !cn.contains(&yr_str) {
                cn = ensure_year_in_clean_name(&cn, yr);
            }
        }
    }

    if let Some(lang) = &schema.language {
        cn = ensure_language_in_clean_name(&cn, lang);
    }

    cn
}

pub fn ensure_year_in_clean_name(name: &str, year: u32) -> String {
    let path = Path::new(name);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("mkv");
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let yr_str = format!("({year})");

    if let Some(bracket_idx) = stem.find('[') {
        let prefix = stem[..bracket_idx].trim_end();
        let suffix = &stem[bracket_idx..];
        format!("{prefix} {yr_str} {suffix}.{ext}")
    } else {
        format!("{stem} {yr_str}.{ext}")
    }
}

pub fn ensure_language_in_clean_name(name: &str, language: &str) -> String {
    let lang_bracket = format!("[{language}]");
    if name.contains(&lang_bracket) {
        return name.to_string();
    }

    let path = Path::new(name);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("mkv");
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);

    for &known in crate::modules::media::probe::KNOWN_LANGUAGES {
        let old_tag = format!("[{known}]");
        if stem.contains(&old_tag) {
            return name.replace(&old_tag, &lang_bracket);
        }
    }
    if stem.contains("[Multi]") {
        return name.replace("[Multi]", &lang_bracket);
    }
    if stem.contains("[Original]") {
        return name.replace("[Original]", &lang_bracket);
    }

    if let Some(bracket_idx) = stem.find('[') {
        let prefix = stem[..bracket_idx].trim_end();
        let suffix = &stem[bracket_idx..];
        format!("{prefix} {lang_bracket} {suffix}.{ext}")
    } else {
        format!("{stem} {lang_bracket}.{ext}")
    }
}

/// Rewrites a clean name so it carries a single canonical resolution tag.
///
/// A pure resolution bracket (`[768p]`, `[1024x768]`) is replaced in place.
/// Combined brackets such as `[x264 AAC]` are left alone; if no resolution
/// bracket exists one is inserted before the first bracket.
pub fn ensure_resolution_in_clean_name(name: &str, resolution: &str) -> String {
    if resolution.trim().is_empty() {
        return name.to_string();
    }
    let target = format!("[{resolution}]");
    if name.contains(&target) {
        return name.to_string();
    }

    let path = Path::new(name);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("mkv");
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);

    if let Some(replaced) = replace_resolution_bracket(stem, resolution) {
        return format!("{replaced}.{ext}");
    }

    if let Some(bracket_idx) = stem.find('[') {
        let prefix = stem[..bracket_idx].trim_end();
        let suffix = &stem[bracket_idx..];
        format!("{prefix} {target} {suffix}.{ext}")
    } else {
        format!("{stem} {target}.{ext}")
    }
}

/// Replaces the first bracket whose whole content is a resolution label.
fn replace_resolution_bracket(stem: &str, canonical: &str) -> Option<String> {
    let bytes = stem.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'[' {
            if let Some(rel) = stem[index + 1..].find(']') {
                let close = index + 1 + rel;
                if crate::modules::media::probe::is_resolution_label(&stem[index + 1..close]) {
                    let mut out = String::with_capacity(stem.len());
                    out.push_str(&stem[..index]);
                    let _ = write!(out, "[{canonical}]");
                    out.push_str(&stem[close + 1..]);
                    return Some(out);
                }
                index = close + 1;
                continue;
            }
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ai_json_injects_year_and_language() -> Result<()> {
        let json_text = r#"{
            "media_type": "movie",
            "title": "Drishyam",
            "year": 2013,
            "season": null,
            "episode": null,
            "resolution": "1080p",
            "language": "Malayalam",
            "clean_name": "Drishyam [1080p].mkv"
        }"#;

        let info = parse_ai_json(json_text, "Drishyam.Malayalam.mkv")?;
        assert_eq!(info.media_type, MediaType::Movie);
        assert_eq!(info.year, Some(2013));
        assert_eq!(info.language.as_deref(), Some("Malayalam"));
        assert_eq!(info.clean_name, "Drishyam (2013) [Malayalam] [1080p].mkv");
        Ok(())
    }

    #[test]
    fn test_parse_batch_ai_json() -> Result<()> {
        let json_text = r#"[
            {
                "raw_name": "Shirokuma.Cafe.01.mkv",
                "media_type": "anime",
                "title": "Shirokuma Cafe",
                "year": null,
                "season": 1,
                "episode": 1,
                "resolution": "1080p",
                "language": "Japanese",
                "is_extra": false,
                "clean_name": "Shirokuma Cafe - S01E01 [Japanese] [1080p].mkv"
            },
            {
                "raw_name": "Shirokuma.Cafe.SP01.mkv",
                "media_type": "anime",
                "title": "Shirokuma Cafe",
                "year": null,
                "season": null,
                "episode": null,
                "resolution": "1080p",
                "language": "Japanese",
                "is_extra": true,
                "clean_name": "Shirokuma Cafe - SP01 [Japanese] [1080p].mkv"
            }
        ]"#;

        let list = parse_batch_ai_json(json_text)?;
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].1.title, "Shirokuma Cafe");
        assert_eq!(list[0].1.episode, Some(1));
        assert!(!list[0].1.is_extra);
        assert!(list[1].1.is_extra);
        Ok(())
    }

    #[test]
    fn test_ensure_resolution_replaces_raw_dimension_bracket() {
        let name = "Show - S01E01 [Japanese] [1024x768].mkv";
        assert_eq!(
            ensure_resolution_in_clean_name(name, "720p"),
            "Show - S01E01 [Japanese] [720p].mkv"
        );
    }

    #[test]
    fn test_ensure_resolution_replaces_p_label_bracket() {
        let name = "Show - S01E01 [Japanese] [768p].mkv";
        assert_eq!(
            ensure_resolution_in_clean_name(name, "720p"),
            "Show - S01E01 [Japanese] [720p].mkv"
        );
    }

    #[test]
    fn test_ensure_resolution_keeps_codec_bracket() {
        let name = "Movie (2026) [Tamil] [1024x768] [x264 AAC].mkv";
        assert_eq!(
            ensure_resolution_in_clean_name(name, "720p"),
            "Movie (2026) [Tamil] [720p] [x264 AAC].mkv"
        );
    }

    #[test]
    fn test_ensure_resolution_inserts_when_absent() {
        let name = "Show - S01E01 [Japanese].mkv";
        assert_eq!(
            ensure_resolution_in_clean_name(name, "1080p"),
            "Show - S01E01 [1080p] [Japanese].mkv"
        );
    }

    #[test]
    fn test_ensure_resolution_is_idempotent() {
        let name = "Movie (2026) [Tamil] [1080p].mkv";
        assert_eq!(
            ensure_resolution_in_clean_name(name, "1080p"),
            "Movie (2026) [Tamil] [1080p].mkv"
        );
    }

    #[test]
    fn test_parse_ai_json_with_markdown_fence() -> Result<()> {
        let fenced_json = "```json\n{\n  \"media_type\": \"movie\",\n  \"title\": \"Kantara\",\n  \"year\": 2022,\n  \"season\": null,\n  \"episode\": null,\n  \"resolution\": \"1080p\",\n  \"language\": \"Kannada\",\n  \"clean_name\": \"Kantara (2022) [Kannada] [1080p].mkv\"\n}\n```";
        let info = parse_ai_json(fenced_json, "Kantara.2022.Kannada.1080p.mkv")?;
        assert_eq!(info.media_type, MediaType::Movie);
        assert_eq!(info.title, "Kantara");
        assert_eq!(info.year, Some(2022));
        assert_eq!(info.language.as_deref(), Some("Kannada"));
        Ok(())
    }
}
