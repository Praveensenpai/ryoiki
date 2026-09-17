pub mod batch;
pub mod client;
pub mod prompt;
pub mod schema;

pub use batch::classify_media_batch;
pub use schema::ensure_language_in_clean_name;

use anyhow::Result;
use reqwest::blocking::Client;

use super::probe::MediaProbe;
use super::MediaInfo;
use client::send_gemini_prompt;
use prompt::build_single_prompt;
use schema::parse_ai_json;

pub fn classify_media_ai(
    client: &Client,
    api_key: &str,
    raw_name: &str,
    probe: Option<&MediaProbe>,
) -> Result<MediaInfo> {
    let prompt = build_single_prompt(raw_name, probe);
    let json_text = send_gemini_prompt(client, api_key, &prompt)?;
    let mut info = parse_ai_json(&json_text, raw_name)?;
    batch::apply_probe_fallback(&mut info, probe);
    Ok(info)
}
