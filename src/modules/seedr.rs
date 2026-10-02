//! Seedr cloud downloader module setup.

use crate::runner::Runner;
use anyhow::Result;
use colored::Colorize;
use std::path::Path;
use std::process::Command;

/// Installs and sets up seedr-dl CLI.
///
/// # Errors
/// Returns an error if installation fails.
pub fn setup(runner: &mut Runner, non_interactive: bool) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let bin_path = Path::new(&home).join(".local/bin/seedr-dl");

    if bin_path.exists() || Runner::command_exists("seedr-dl") {
        println!("  {} seedr-dl is already installed.", "✔".green());
    } else {
        runner.exec_bash(
            "Installing seedr-dl (Praveensenpai/seedr-dl)...",
            "curl -fsSL https://raw.githubusercontent.com/Praveensenpai/seedr-dl/main/install.sh | bash",
        )?;
    }

    let auth_path = Path::new(&home).join(".config/seedr-dl/auth.json");
    if !auth_path.exists() && !non_interactive {
        println!();
        println!(
            "  {} Authenticating Seedr.cc for fast cloud downloads...",
            "🌱".cyan()
        );
        let exe = if bin_path.exists() {
            bin_path.to_string_lossy().to_string()
        } else {
            "seedr-dl".to_string()
        };
        let _ = Command::new(exe).arg("auth").status();
    }

    Ok(())
}
