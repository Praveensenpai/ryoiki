pub mod ai;
pub mod config;
pub mod disk;
pub mod heuristic;
pub mod interactive;
pub mod organizer;
pub mod pruner;
pub mod transfer;

use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType {
    Movie,
    Show,
}

impl std::fmt::Display for MediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Movie => write!(f, "Movie"),
            Self::Show => write!(f, "TV Show"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClassificationEngine {
    Ai,
    Heuristic,
}

impl std::fmt::Display for ClassificationEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ai => write!(f, "🤖 Gemini AI"),
            Self::Heuristic => write!(f, "⚡ Heuristic Fallback"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub media_type: MediaType,
    pub title: String,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub resolution: Option<String>,
    pub clean_name: String,
    pub engine: ClassificationEngine,
}

#[derive(Debug, Clone)]
pub struct OrganizeResult {
    pub source_path: PathBuf,
    pub dest_path: PathBuf,
    pub media_info: MediaInfo,
}

/// Runs the interactive media manager TUI to push or pull media between SSD and Google Drive.
pub fn handle_media_cli(action: Option<&str>) -> anyhow::Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let home_path = Path::new(&home);

    if !std::io::stdin().is_terminal() {
        println!(
            "\n  {} Interactive media manager requires an interactive terminal (TTY).\n",
            "✖".red().bold()
        );
        return Ok(());
    }

    let default_dir = match action {
        Some("pull") => transfer::TransferDirection::Pull,
        _ => transfer::TransferDirection::Push,
    };

    println!(
        "\n  {} Scanning local and cloud media libraries...",
        "▶".cyan().bold()
    );
    let local_items = transfer::scan_local_media(home_path);
    let remote_items = transfer::scan_remote_media(home_path)?;

    let media_base = home_path.join("jellyfin/media");
    let check_path = if media_base.exists() {
        &media_base
    } else {
        home_path
    };
    let local_disk = disk::get_disk_usage(check_path).ok();

    let res = interactive::run_interactive_tui(local_items, remote_items, default_dir, local_disk)?;
    let Some((dir, selected)) = res else {
        println!("\n  {} Transfer cancelled.\n", "•".dimmed());
        return Ok(());
    };

    if transfer::confirm_transfer(dir, &selected) {
        transfer::execute_transfer(dir, &selected, home_path)?;
    } else {
        println!("\n  {} Transfer cancelled by user.\n", "•".dimmed());
    }
    Ok(())
}
