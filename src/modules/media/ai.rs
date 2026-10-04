pub mod batch;
pub mod client;
pub mod prompt;
pub mod schema;

pub use batch::classify_media_batch;
#[allow(unused_imports)]
pub use client::{clean_json_text, send_ai_prompt, send_deepseek_prompt, send_gemini_prompt};
pub use schema::ensure_language_in_clean_name;

use anyhow::Result;
use reqwest::blocking::Client;

use super::probe::MediaProbe;
use super::MediaInfo;
use prompt::build_single_prompt;
use schema::parse_ai_json;

pub fn classify_media_ai(
    client: &Client,
    api_key: Option<&str>,
    raw_name: &str,
    probe: Option<&MediaProbe>,
) -> Result<MediaInfo> {
    let prompt = build_single_prompt(raw_name, probe);
    let json_text = send_ai_prompt(client, api_key, &prompt)?;
    let mut info = parse_ai_json(&json_text, raw_name)?;
    batch::apply_probe_fallback(&mut info, probe);
    Ok(info)
}
