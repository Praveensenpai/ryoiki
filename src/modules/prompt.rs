use crate::runner::Runner;
use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn setup(runner: &mut Runner) -> Result<()> {
    // 1. Starship prompt
    if !Runner::command_exists("starship") {
        runner.exec_bash(
            "Installing Starship prompt...",
            "curl -sS https://starship.rs/install.sh | sh -s -- -y",
        )?;
    }

    // 2. Fastfetch (standalone CLI utility, not auto-run on terminal launch)
    runner.apt_install("Installing Fastfetch CLI...", &["fastfetch"])?;

    // 3. Clean any legacy auto-executing fastfetch hook from ~/.bashrc
    cleanup_fastfetch_hook()?;

    Ok(())
}

fn cleanup_fastfetch_hook() -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let bashrc_path = Path::new(&home).join(".bashrc");
    if bashrc_path.exists() {
        let content = fs::read_to_string(&bashrc_path)?;
        if content.contains("fastfetch") {
            let cleaned: Vec<&str> = content
                .lines()
                .filter(|line| !line.contains("fastfetch") && !line.contains("Display fastfetch"))
                .collect();
            fs::write(&bashrc_path, cleaned.join("\n") + "\n")?;
        }
    }
    Ok(())
}
