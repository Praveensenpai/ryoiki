use anyhow::{Context, Result};
use serde::Deserialize;
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
    let schema: AiOutputSchema =
        serde_json::from_str(json_text).context("Failed to parse model JSON into schema")?;
    Ok(build_media_info(&schema, raw_name))
}

pub fn parse_batch_ai_json(json_text: &str) -> Result<Vec<(String, MediaInfo)>> {
    let items: Vec<AiBatchItemSchema> =
        serde_json::from_str(json_text).context("Failed to parse batch JSON array")?;

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

    if let Some(bracket_idx) = stem.find('[') {
        let prefix = stem[..bracket_idx].trim_end();
        let suffix = &stem[bracket_idx..];
        format!("{prefix} {lang_bracket} {suffix}.{ext}")
    } else {
        format!("{stem} {lang_bracket}.{ext}")
    }
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
}
