use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;
use std::thread::sleep;
use std::time::Duration;

pub const GEMINI_MODEL: &str = "gemini-3.1-flash-lite";
const RETRY_DELAYS: [u64; 6] = [1, 2, 5, 10, 15, 30];

/// Strips markdown code blocks (e.g. ```json ... ```) from AI response text.
#[must_use]
pub fn clean_json_text(text: &str) -> String {
    let mut cleaned = text.trim();
    if cleaned.starts_with("```json") {
        cleaned = cleaned.trim_start_matches("```json");
    } else if cleaned.starts_with("```") {
        cleaned = cleaned.trim_start_matches("```");
    }
    if cleaned.ends_with("```") {
        cleaned = cleaned.trim_end_matches("```");
    }
    cleaned.trim().to_string()
}

/// Unified AI caller:
/// 1. Tries primary `DeepSeek` provider first (if enabled).
/// 2. Falls back to Gemini secondary provider if `DeepSeek` fails or is disabled.
pub fn send_ai_prompt(
    client: &Client,
    gemini_api_key: Option<&str>,
    prompt: &str,
) -> Result<String> {
    if crate::modules::media::config::is_deepseek_enabled() {
        match send_deepseek_prompt(client, prompt) {
            Ok(res) => return Ok(res),
            Err(e) => {
                eprintln!("  ℹ DeepSeek API failed ({e}), falling back to Gemini...");
            }
        }
    }

    let key = gemini_api_key
        .map(std::string::ToString::to_string)
        .or_else(|| crate::modules::media::config::get_or_prompt_gemini_key(false));

    if let Some(key) = key {
        send_gemini_prompt_inner(client, &key, prompt)
    } else {
        bail!("No AI provider available: DeepSeek failed and no Gemini API key configured.")
    }
}

pub fn send_deepseek_prompt(client: &Client, prompt: &str) -> Result<String> {
    let url = crate::modules::media::config::get_deepseek_url();
    let model = crate::modules::media::config::get_deepseek_model();
    let api_key = crate::modules::media::config::get_deepseek_api_key();

    let body = json!({
        "model": model,
        "messages": [
            { "role": "user", "content": prompt }
        ],
        "temperature": 0.1
    });

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .timeout(Duration::from_secs(25))
        .json(&body)
        .send()
        .with_context(|| format!("DeepSeek HTTP request failed at {url}"))?;

    let status = resp.status();
    if !status.is_success() {
        let err_text = resp.text().unwrap_or_default();
        bail!("DeepSeek returned status {status}: {err_text}");
    }

    let parsed: serde_json::Value = resp.json().context("Failed to deserialize DeepSeek JSON")?;
    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .context("DeepSeek response missing choices[0].message.content")?;

    Ok(clean_json_text(content))
}

#[allow(dead_code)]
pub fn send_gemini_prompt(client: &Client, api_key: &str, prompt: &str) -> Result<String> {
    send_ai_prompt(client, Some(api_key), prompt)
}

fn send_gemini_prompt_inner(client: &Client, api_key: &str, prompt: &str) -> Result<String> {
    let configured_model = crate::modules::media::config::get_gemini_model();
    let candidates = [
        configured_model.as_str(),
        GEMINI_MODEL,
        "gemini-3.5-flash-lite",
    ];

    let mut last_err = String::from("No response received");

    for model in candidates {
        for (attempt, &delay_secs) in [0].iter().chain(RETRY_DELAYS.iter()).enumerate() {
            if attempt > 0 {
                eprintln!(
                    "  ⚠️ Gemini API request failed ({model}, attempt {attempt}/{}), retrying in {delay_secs}s...",
                    RETRY_DELAYS.len()
                );
                sleep(Duration::from_secs(delay_secs));
            }

            match send_gemini_request(client, model, api_key, prompt) {
                Ok(json_text) => return Ok(clean_json_text(&json_text)),
                Err((status, e)) => {
                    last_err = format!("{model}: {e}");
                    if status.is_client_error() && status != reqwest::StatusCode::TOO_MANY_REQUESTS {
                        break;
                    }
                }
            }
        }
    }

    bail!("Gemini API failed: {last_err}")
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_json_text() {
        assert_eq!(clean_json_text("```json\n{\"test\": 1}\n```"), "{\"test\": 1}");
        assert_eq!(clean_json_text("```\n{\"test\": 2}\n```"), "{\"test\": 2}");
        assert_eq!(clean_json_text("{\"test\": 3}"), "{\"test\": 3}");
    }
}
