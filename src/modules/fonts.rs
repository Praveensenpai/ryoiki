use crate::runner::Runner;
use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Installs `JetBrainsMono` Nerd Font to user fonts directory and updates font cache.
pub fn setup(runner: &mut Runner) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let font_dir = Path::new(&home).join(".local/share/fonts/NerdFonts");
    let marker_file = font_dir.join("JetBrainsMonoNerdFont-Regular.ttf");

    if marker_file.exists() {
        println!(
            "  {} JetBrainsMono Nerd Font is already installed.",
            "✔".green()
        );
    } else {
        fs::create_dir_all(&font_dir)?;

        runner.exec_bash(
            "Downloading and installing JetBrainsMono Nerd Font...",
            r#"
            FONT_DIR="$HOME/.local/share/fonts/NerdFonts"
            mkdir -p "$FONT_DIR"
            curl -fsSL "https://github.com/ryanoasis/nerd-fonts/releases/latest/download/JetBrainsMono.tar.xz" | tar -xJ -C "$FONT_DIR"
            "#,
        )?;
    }

    // Refresh font cache if fc-cache is available
    if Runner::command_exists("fc-cache") {
        runner.exec_silent(
            "Updating font cache (fc-cache)...",
            "fc-cache",
            &["-f", font_dir.to_str().unwrap_or("~/.local/share/fonts")],
        )?;
    }

    println!(
        "  {} Set your local terminal font to: {}",
        "Tip:".dimmed(),
        "JetBrainsMono Nerd Font".cyan().bold()
    );

    Ok(())
}
