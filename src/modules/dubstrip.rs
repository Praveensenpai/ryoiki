use crate::runner::Runner;
use anyhow::Result;
use colored::Colorize;
use std::io::{self, BufRead, Write};
use std::process::Command;

pub fn setup(runner: &mut Runner, non_interactive: bool) -> Result<()> {
    if Runner::command_exists("dubstrip") {
        println!("  dubstrip is already installed.");
    } else {
        runner.exec_bash(
            "Installing dubstrip (Praveensenpai/dubstrip)...",
            "curl -fsSL -H \"Cache-Control: no-cache\" https://raw.githubusercontent.com/Praveensenpai/dubstrip/main/install.sh | bash",
        )?;
    }

    if !non_interactive {
        prompt_gemini_key_if_missing()?;
    }

    Ok(())
}

fn prompt_gemini_key_if_missing() -> Result<()> {
    if crate::modules::media::config::get_or_prompt_gemini_key(false).is_some() {
        println!("  {} Gemini API key is already configured.", "✔".green());
        return Ok(());
    }

    println!();
    println!(
        "  {} Gemini AI: Enhances movie theatrical origin disambiguation in DubStrip.",
        "🤖".cyan()
    );
    println!("     Get a free key at: https://aistudio.google.com/");
    print!("  Enter Gemini API Key [press Enter to skip]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    let key = trimmed.to_string();
    if let Ok(mut cfg) = crate::notify::TelegramConfig::load() {
        cfg.gemini_api_key = Some(key.clone());
        let _ = cfg.save();
    }

    if Runner::command_exists("dubstrip") {
        let _ = Command::new("dubstrip")
            .args(["config", "--set-key", &key])
            .output();
    }

    println!(
        "  {} Saved Gemini API key to ryoiki & dubstrip configuration.\n",
        "✔".green().bold()
    );
    Ok(())
}
