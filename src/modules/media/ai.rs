use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;
use std::thread::sleep;
use std::time::Duration;

use super::{ClassificationEngine, MediaInfo, MediaType};

const RETRY_DELAYS: [u64; 6] = [1, 2, 5, 10, 15, 30];
const GEMINI_MODELS: &[&str] = &["gemini-3.6-flash", "gemini-flash-latest"];

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<Candidate>>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: Option<CandidateContent>,
}

#[derive(Debug, Deserialize)]
struct CandidateContent {
    parts: Option<Vec<Part>>,
}

#[derive(Debug, Deserialize)]
struct Part {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AiOutputSchema {
    media_type: String,
    title: String,
    year: Option<u32>,
    season: Option<u32>,
    episode: Option<u32>,
    resolution: Option<String>,
    language: Option<String>,
    clean_name: Option<String>,
}

pub fn classify_media_ai(
    client: &Client,
    api_key: &str,
    raw_name: &str,
    probe: Option<&super::probe::MediaProbe>,
) -> Result<MediaInfo> {
    let prompt = build_prompt(raw_name, probe);
    let mut last_err = String::from("No response received");

    for &model in GEMINI_MODELS {
        for (attempt, &delay_secs) in [0].iter().chain(RETRY_DELAYS.iter()).enumerate() {
            if attempt > 0 {
                eprintln!(
                    "  ⚠️ Gemini API request failed ({model}, attempt {attempt}/{}), retrying in {delay_secs}s...",
                    RETRY_DELAYS.len()
                );
                sleep(Duration::from_secs(delay_secs));
            }

            match send_gemini_request(client, model, api_key, &prompt) {
                Ok(json_text) => {
                    if let Ok(mut info) = parse_ai_json(&json_text, raw_name) {
                        apply_probe_fallback(&mut info, probe);
                        return Ok(info);
                    }
                }
                Err((status, e)) => {
                    last_err = format!("{model}: {e}");
                    if status.is_client_error() && status != reqwest::StatusCode::TOO_MANY_REQUESTS
                    {
                        break;
                    }
                }
            }
        }
    }

    bail!("Gemini API failed: {last_err}")
}

fn apply_probe_fallback(info: &mut MediaInfo, probe: Option<&super::probe::MediaProbe>) {
    if info.language.is_none() {
        if let Some(p) = probe {
            info.language.clone_from(&p.primary_language);
        }
    }
    if let Some(lang) = &info.language {
        info.clean_name = ensure_language_in_clean_name(&info.clean_name, lang);
    }
}

fn build_prompt(raw_name: &str, probe: Option<&super::probe::MediaProbe>) -> String {
    let probe_context = format_probe_context(probe);
    format!(
        "You are an expert media organizer for Jellyfin. Given this messy torrent/file name:\n\
        \"{raw_name}\"\n\
        {probe_context}\
        CRITICAL INSTRUCTIONS:\n\
        1. Identify remakes/versions: If multiple movies exist with this title (e.g. original vs regional remakes like Drishyam 2013 Malayalam vs Drishyam 2015 Hindi), use the detected audio language, media duration, and cinema knowledge to identify the EXACT film version.\n\
        2. Release Year is MANDATORY for movies: Always provide the 4-digit release year for movies in both the 'year' field and inside 'clean_name' (e.g. \"Title (YEAR) [Language] [1080p].ext\"). If missing in the filename, deduce the correct year from the title, runtime duration, and audio language.\n\
        3. Clean title: Strip release groups, years, audio info, languages, and site tags from 'title'. Keep title clean.\n\
        \n\
        Parse and return JSON with keys:\n\
        - media_type: \"movie\" or \"show\"\n\
        - title: clean title without release group, websites, resolution, or year\n\
        - year: integer release year (e.g. 2013) or null\n\
        - season: integer season number or null (if show)\n\
        - episode: integer episode number or null (if show)\n\
        - resolution: string e.g. \"1080p\", \"2160p\", \"720p\" or null\n\
        - language: string primary audio language capitalized (e.g. \"Malayalam\", \"English\", \"Tamil\", \"Hindi\", \"Multi\") or null\n\
        - clean_name: formatted filename with original file extension (e.g. \"Title (2013) [Malayalam] [1080p].mkv\" or \"Title - S01E02 [English] [1080p].mkv\")"
    )
}

fn format_probe_context(probe: Option<&super::probe::MediaProbe>) -> String {
    let Some(p) = probe else {
        return String::new();
    };

    let mut details = Vec::new();
    if let Some(mins) = p.duration_mins {
        details.push(format!("Exact media duration: ~{mins} minutes"));
    }
    if !p.audio_languages.is_empty() {
        details.push(format!(
            "Audio language tracks detected: {}",
            p.audio_languages.join(", ")
        ));
    }
    if let Some(res) = &p.resolution {
        details.push(format!("Detected stream resolution: {res}"));
    }

    if details.is_empty() {
        String::new()
    } else {
        format!(
            "\nPhysical media file stream probe:\n{}\n\n",
            details
                .iter()
                .map(|d| format!("- {d}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

fn send_gemini_request(
    client: &Client,
    model: &str,
    api_key: &str,
    prompt: &str,
) -> std::result::Result<String, (reqwest::StatusCode, anyhow::Error)> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={api_key}"
    );

    let body = json!({
        "contents": [{
            "parts": [{ "text": prompt }]
        }],
        "generationConfig": {
            "response_mime_type": "application/json"
        }
    });

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .map_err(|e| (reqwest::StatusCode::INTERNAL_SERVER_ERROR, e.into()))?;

    let status = resp.status();
    if !status.is_success() {
        return Err((status, anyhow::anyhow!("API status {status}")));
    }

    let parsed: GeminiResponse = resp
        .json()
        .map_err(|e| (reqwest::StatusCode::INTERNAL_SERVER_ERROR, e.into()))?;
    extract_part_text(parsed).map_err(|e| (reqwest::StatusCode::INTERNAL_SERVER_ERROR, e))
}

fn extract_part_text(resp: GeminiResponse) -> Result<String> {
    let candidate = resp
        .candidates
        .and_then(|mut c| {
            if c.is_empty() {
                None
            } else {
                Some(c.remove(0))
            }
        })
        .context("No candidate in Gemini response")?;

    let content = candidate.content.context("Missing candidate content")?;
    let part = content
        .parts
        .and_then(|mut p| {
            if p.is_empty() {
                None
            } else {
                Some(p.remove(0))
            }
        })
        .context("Missing content parts in candidate")?;

    part.text.context("Empty text in Gemini response part")
}

fn parse_ai_json(json_text: &str, raw_name: &str) -> Result<MediaInfo> {
    let schema: AiOutputSchema =
        serde_json::from_str(json_text).context("Failed to parse model JSON into schema")?;

    let media_type = if schema.media_type.eq_ignore_ascii_case("show") {
        MediaType::Show
    } else {
        MediaType::Movie
    };

    let clean_name = resolve_clean_name(&schema, media_type, raw_name);

    Ok(MediaInfo {
        media_type,
        title: schema.title,
        year: schema.year,
        season: schema.season,
        episode: schema.episode,
        resolution: schema.resolution,
        language: schema.language,
        clean_name,
        engine: ClassificationEngine::Ai,
    })
}

fn resolve_clean_name(schema: &AiOutputSchema, media_type: MediaType, raw_name: &str) -> String {
    let ext = std::path::Path::new(raw_name)
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

    if media_type == MediaType::Movie {
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

fn ensure_year_in_clean_name(name: &str, year: u32) -> String {
    let path = std::path::Path::new(name);
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

    let path = std::path::Path::new(name);
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
    fn test_ensure_year_in_clean_name() {
        assert_eq!(
            ensure_year_in_clean_name("Drishyam [1080p].mkv", 2013),
            "Drishyam (2013) [1080p].mkv"
        );
        assert_eq!(
            ensure_year_in_clean_name("Drishyam.mkv", 2013),
            "Drishyam (2013).mkv"
        );
    }

    #[test]
    fn test_ensure_language_in_clean_name() {
        assert_eq!(
            ensure_language_in_clean_name("Drishyam (2013) [1080p].mkv", "Malayalam"),
            "Drishyam (2013) [Malayalam] [1080p].mkv"
        );
        assert_eq!(
            ensure_language_in_clean_name("Drishyam (2013).mkv", "Malayalam"),
            "Drishyam (2013) [Malayalam].mkv"
        );
    }
}
