use anyhow::Result;
use reqwest::blocking::Client;
use std::collections::HashMap;

use super::super::probe::MediaProbe;
use super::super::MediaInfo;
use super::client::send_gemini_prompt;
use super::prompt::build_batch_prompt;
use super::schema::parse_batch_ai_json;

const BATCH_CHUNK_SIZE: usize = 35;

pub fn classify_media_batch(
    client: &Client,
    api_key: &str,
    items: &[(&str, Option<&MediaProbe>)],
) -> Result<HashMap<String, MediaInfo>> {
    let mut mapped_results = HashMap::with_capacity(items.len());

    for chunk in items.chunks(BATCH_CHUNK_SIZE) {
        let raw_names: Vec<&str> = chunk.iter().map(|(n, _)| *n).collect();
        let rep_probe = chunk.iter().find_map(|(_, p)| *p);

        let prompt = build_batch_prompt(&raw_names, rep_probe);
        let json_text = send_gemini_prompt(client, api_key, &prompt)?;

        let parsed_list = parse_batch_ai_json(&json_text)?;
        for (raw, mut info) in parsed_list {
            if let Some((_, probe)) = chunk.iter().find(|(n, _)| **n == raw) {
                apply_probe_fallback(&mut info, *probe);
            }
            mapped_results.insert(raw, info);
        }
    }

    Ok(mapped_results)
}

pub fn apply_probe_fallback(info: &mut MediaInfo, probe: Option<&MediaProbe>) {
    if info.language.is_none() {
        if let Some(p) = probe {
            info.language.clone_from(&p.primary_language);
        }
    }
    if let Some(lang) = &info.language {
        info.clean_name = super::schema::ensure_language_in_clean_name(&info.clean_name, lang);
    }
}
