use anyhow::Result;
use colored::Colorize;
use std::io::{self, BufRead, Write};

use crate::notify::TelegramConfig;

/// Resolves the Gemini API key from environment, config file, or optional interactive prompt.
pub fn get_or_prompt_gemini_key(interactive: bool) -> Option<String> {
    if let Ok(key) = std::env::var("GEMINI_API_KEY") {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    if let Ok(cfg) = TelegramConfig::load() {
        if let Some(key) = cfg.gemini_api_key {
            let trimmed = key.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    if !interactive {
        return None;
    }

    prompt_and_save_key().ok().flatten()
}

fn prompt_and_save_key() -> Result<Option<String>> {
    print!(
        "  {} Enter Gemini API Key for AI media classification [skip]: ",
        "🤖".cyan()
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let key = trimmed.to_string();
    if let Ok(mut cfg) = TelegramConfig::load() {
        cfg.gemini_api_key = Some(key.clone());
        let _ = cfg.save();
    }
    println!(
        "  {} Saved Gemini API key to configuration",
        "✔".green().bold()
    );
    Ok(Some(key))
}

/// Resolves whether `DeepSeek` is enabled (defaults to true).
#[must_use]
pub fn is_deepseek_enabled() -> bool {
    if let Ok(val) = std::env::var("ENABLE_DEEPSEEK") {
        let lower = val.trim().to_lowercase();
        return lower == "1" || lower == "true" || lower == "yes";
    }
    TelegramConfig::load()
        .ok()
        .and_then(|c| c.enable_deepseek)
        .unwrap_or(true)
}

/// Resolves the `DeepSeek` URL (defaults to `<http://mochi:4000/v1/chat/completions>`).
#[must_use]
pub fn get_deepseek_url() -> String {
    if let Ok(url) = std::env::var("DEEPSEEK_URL") {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    TelegramConfig::load()
        .ok()
        .and_then(|c| c.deepseek_url)
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| "http://mochi:4000/v1/chat/completions".to_string())
}

/// Resolves the `DeepSeek` model (defaults to v4.1flash).
#[must_use]
pub fn get_deepseek_model() -> String {
    if let Ok(model) = std::env::var("DEEPSEEK_MODEL") {
        let trimmed = model.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    TelegramConfig::load()
        .ok()
        .and_then(|c| c.deepseek_model)
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| "v4.1flash".to_string())
}

/// Resolves the `DeepSeek` API key (defaults to dseeker).
#[must_use]
pub fn get_deepseek_api_key() -> String {
    if let Ok(key) =
        std::env::var("DEEPSEEK_API_KEY").or_else(|_| std::env::var("DEEPSEEKER_API_KEY"))
    {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    TelegramConfig::load()
        .ok()
        .and_then(|c| c.deepseek_api_key)
        .filter(|k| !k.trim().is_empty())
        .unwrap_or_else(|| "dseeker".to_string())
}

/// Resolves the Gemini model (defaults to gemini-3.1-flash-lite).
#[must_use]
pub fn get_gemini_model() -> String {
    if let Ok(model) = std::env::var("GEMINI_MODEL") {
        let trimmed = model.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    TelegramConfig::load()
        .ok()
        .and_then(|c| c.gemini_model)
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| "gemini-3.1-flash-lite".to_string())
}

/// Checks whether any AI engine is enabled or configured.
#[must_use]
pub fn is_ai_enabled() -> bool {
    is_deepseek_enabled() || get_or_prompt_gemini_key(false).is_some()
}
