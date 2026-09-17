use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;
use std::thread::sleep;
use std::time::Duration;

pub const GEMINI_MODEL: &str = "gemini-3.5-flash-lite";
const RETRY_DELAYS: [u64; 6] = [1, 2, 5, 10, 15, 30];

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

pub fn send_gemini_prompt(client: &Client, api_key: &str, prompt: &str) -> Result<String> {
    let mut last_err = String::from("No response received");

    for (attempt, &delay_secs) in [0].iter().chain(RETRY_DELAYS.iter()).enumerate() {
        if attempt > 0 {
            eprintln!(
                "  ⚠️ Gemini API request failed ({GEMINI_MODEL}, attempt {attempt}/{}), retrying in {delay_secs}s...",
                RETRY_DELAYS.len()
            );
            sleep(Duration::from_secs(delay_secs));
        }

        match send_gemini_request(client, GEMINI_MODEL, api_key, prompt) {
            Ok(json_text) => return Ok(json_text),
            Err((status, e)) => {
                last_err = format!("{GEMINI_MODEL}: {e}");
                if status.is_client_error() && status != reqwest::StatusCode::TOO_MANY_REQUESTS {
                    break;
                }
            }
        }
    }

    bail!("Gemini API failed: {last_err}")
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
