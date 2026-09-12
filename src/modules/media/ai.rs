use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;
use std::thread::sleep;
use std::time::Duration;

use super::{ClassificationEngine, MediaInfo, MediaType};

const RETRY_DELAYS: [u64; 6] = [1, 2, 5, 15, 30, 60];
const GEMINI_MODEL: &str = "gemini-2.5-flash";

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
    clean_name: Option<String>,
}

pub fn classify_media_ai(client: &Client, api_key: &str, raw_name: &str) -> Result<MediaInfo> {
    let prompt = build_prompt(raw_name);
    let mut last_err = String::from("No response received");

    for (attempt, &delay_secs) in [0].iter().chain(RETRY_DELAYS.iter()).enumerate() {
        if attempt > 0 {
            eprintln!(
                "  ⚠️ Gemini API request failed (attempt {attempt}/6), retrying in {delay_secs}s..."
            );
            sleep(Duration::from_secs(delay_secs));
        }

        match send_gemini_request(client, api_key, &prompt) {
            Ok(json_text) => {
                if let Ok(info) = parse_ai_json(&json_text, raw_name) {
                    return Ok(info);
                }
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
    }

    bail!(
        "Gemini API failed after {} retries: {last_err}",
        RETRY_DELAYS.len()
    )
}

fn build_prompt(raw_name: &str) -> String {
    format!(
        "You are an expert media organizer for Jellyfin. Given this messy torrent/file name:\n\
        \"{raw_name}\"\n\
        Parse and return JSON with keys:\n\
        - media_type: \"movie\" or \"show\"\n\
        - title: clean title without release group, websites, resolution, or year\n\
        - year: integer release year (e.g. 2026) or null\n\
        - season: integer season number or null (if show)\n\
        - episode: integer episode number or null (if show)\n\
        - resolution: string e.g. \"1080p\", \"2160p\", \"720p\" or null\n\
        - clean_name: formatted filename with original file extension (e.g. \"Title (2026) [1080p].mkv\" or \"Title - S01E02 [1080p].mkv\")"
    )
}

fn send_gemini_request(client: &Client, api_key: &str, prompt: &str) -> Result<String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{GEMINI_MODEL}:generateContent?key={api_key}"
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
        .context("HTTP request to Gemini API failed")?;

    if !resp.status().is_success() {
        bail!("Gemini API returned status {}", resp.status());
    }

    let parsed: GeminiResponse = resp
        .json()
        .context("Failed to parse Gemini response JSON")?;
    extract_part_text(parsed)
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

    let clean_name = schema.clean_name.unwrap_or_else(|| {
        let ext = std::path::Path::new(raw_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mkv");
        match (media_type, schema.year) {
            (MediaType::Movie, Some(yr)) => format!("{} ({yr}).{ext}", schema.title),
            _ => format!("{}.{ext}", schema.title),
        }
    });

    Ok(MediaInfo {
        media_type,
        title: schema.title,
        year: schema.year,
        season: schema.season,
        episode: schema.episode,
        resolution: schema.resolution,
        clean_name,
        engine: ClassificationEngine::Ai,
    })
}
