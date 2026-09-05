use crate::runner::Runner;
use anyhow::Result;

pub fn setup(runner: &mut Runner) -> Result<()> {
    runner.apt_update()?;

    runner.apt_install(
        "Installing prerequisites (curl, wget, ca-certificates, gnupg)...",
        &["curl", "wget", "ca-certificates", "gnupg"],
    )?;

    runner.apt_install(
        "Installing essentials (git, tmux, neovim)...",
        &["git", "tmux", "neovim"],
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

    Ok(())
}
