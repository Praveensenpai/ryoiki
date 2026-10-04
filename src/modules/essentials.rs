use crate::runner::Runner;
use anyhow::Result;
use colored::Colorize;
use std::path::Path;

/// Installs essential tools including git, tmux, neovim, adb, official GitHub CLI,
/// and the `OpenCode` AI coding agent.
pub fn setup(runner: &mut Runner) -> Result<()> {
    runner.apt_update()?;

    runner.apt_install(
        "Installing prerequisites (curl, wget, ca-certificates, gnupg)...",
        &["curl", "wget", "ca-certificates", "gnupg"],
    )?;

    runner.apt_install(
        "Installing essentials (git, tmux, neovim, adb)...",
        &["git", "tmux", "neovim", "adb"],
    )?;

    if !Runner::command_exists("gh") {
        runner.exec_bash(
            "Setting up official GitHub CLI (gh) repository...",
            r#"
            sudo mkdir -p -m 755 /etc/apt/keyrings
            wget -qO- https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo tee /etc/apt/keyrings/githubcli-archive-keyring.gpg > /dev/null
            sudo chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg
            echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null
            sudo apt-get update -y
            sudo apt-get install -y gh
            "#,
        )?;
    }

    install_opencode(runner)?;

    Ok(())
}

/// Installs the `OpenCode` AI coding agent CLI if it is not already present.
fn install_opencode(runner: &mut Runner) -> Result<()> {
    if opencode_installed() {
        println!("  {} opencode is already installed.", "✔".green());
        return Ok(());
    }

    runner.exec_bash(
        "Installing OpenCode AI coding agent (opencode.ai)...",
        "curl -fsSL https://opencode.ai/install | bash",
    )
}

/// Returns true when the `OpenCode` binary is in PATH or its default install dir.
fn opencode_installed() -> bool {
    if Runner::command_exists("opencode") {
        return true;
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    Path::new(&home).join(".opencode/bin/opencode").exists()
}
