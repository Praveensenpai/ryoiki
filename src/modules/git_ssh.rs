use crate::runner::Runner;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// Configures Git global identity and SSH key onboarding for GitHub.
pub fn setup(runner: &mut Runner, non_interactive: bool) -> Result<()> {
    let git_email = setup_git_identity(runner, non_interactive)?;
    let pubkey_path = setup_ssh_key(runner, &git_email)?;

    if let Ok(pubkey) = fs::read_to_string(&pubkey_path) {
        println!("\n  {}", "─── Your GitHub Public SSH Key ───".dimmed());
        println!("  {}", pubkey.trim().cyan().bold());
        println!("  {}", "───────────────────────────────────".dimmed());
        println!(
            "  Paste into: {}\n",
            "https://github.com/settings/ssh/new".underline().blue()
        );
    }

    if !non_interactive && !runner.dry_run {
        verify_github_ssh();
    }

    Ok(())
}

fn setup_git_identity(runner: &mut Runner, non_interactive: bool) -> Result<String> {
    let (current_user, current_email) = read_current_git_config();

    let (git_user, git_email) = if non_interactive || runner.dry_run {
        let user = if current_user.is_empty() {
            "Praveensenpai".to_string()
        } else {
            current_user
        };
        let email = if current_email.is_empty() {
            "pvnt20@gmail.com".to_string()
        } else {
            current_email
        };
        (user, email)
    } else {
        prompt_git_identity(&current_user, &current_email)?
    };

    apply_git_identity(runner, &git_user, &git_email)?;
    Ok(git_email)
}

fn read_current_git_config() -> (String, String) {
    let current_user = std::process::Command::new("git")
        .args(["config", "--global", "user.name"])
        .output()
        .map_or_else(
            |_| String::new(),
            |o| String::from_utf8_lossy(&o.stdout).trim().to_string(),
        );

    let current_email = std::process::Command::new("git")
        .args(["config", "--global", "user.email"])
        .output()
        .map_or_else(
            |_| String::new(),
            |o| String::from_utf8_lossy(&o.stdout).trim().to_string(),
        );

    (current_user, current_email)
}

fn apply_git_identity(runner: &mut Runner, git_user: &str, git_email: &str) -> Result<()> {
    runner.exec_silent(
        &format!("Configuring Git identity: {git_user} <{git_email}>"),
        "git",
        &["config", "--global", "user.name", git_user],
    )?;
    runner.exec_silent(
        "Setting Git global email",
        "git",
        &["config", "--global", "user.email", git_email],
    )?;
    runner.exec_silent(
        "Setting Git default branch to main",
        "git",
        &["config", "--global", "init.defaultBranch", "main"],
    )?;
    Ok(())
}

fn prompt_git_identity(current_user: &str, current_email: &str) -> Result<(String, String)> {
    let default_user = if current_user.is_empty() {
        "Praveensenpai"
    } else {
        current_user
    };
    let default_email = if current_email.is_empty() {
        "pvnt20@gmail.com"
    } else {
        current_email
    };

    print!("  Enter Git username [{}]: ", default_user.cyan());
    io::stdout().flush()?;
    let mut input_user = String::new();
    io::stdin().lock().read_line(&mut input_user)?;
    let input_user = input_user.trim();
    let final_user = if input_user.is_empty() {
        default_user
    } else {
        input_user
    };

    print!("  Enter Git email [{}]: ", default_email.cyan());
    io::stdout().flush()?;
    let mut input_email = String::new();
    io::stdin().lock().read_line(&mut input_email)?;
    let input_email = input_email.trim();
    let final_email = if input_email.is_empty() {
        default_email
    } else {
        input_email
    };

    Ok((final_user.to_string(), final_email.to_string()))
}

fn setup_ssh_key(runner: &mut Runner, git_email: &str) -> Result<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let ssh_dir = Path::new(&home).join(".ssh");
    let key_path = ssh_dir.join("id_ed25519");
    let pubkey_path = ssh_dir.join("id_ed25519.pub");

    if pubkey_path.exists() {
        println!(
            "  {} Existing SSH key found: {}",
            "✔".green(),
            pubkey_path.display().to_string().dimmed()
        );
    } else {
        fs::create_dir_all(&ssh_dir)?;
        let key_str = key_path.to_str().context("Invalid SSH key path")?;
        runner.exec_silent(
            "Generating Ed25519 SSH Key...",
            "ssh-keygen",
            &["-t", "ed25519", "-C", git_email, "-f", key_str, "-N", ""],
        )?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&ssh_dir, fs::Permissions::from_mode(0o700));
        if key_path.exists() {
            let _ = fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600));
        }
        if pubkey_path.exists() {
            let _ = fs::set_permissions(&pubkey_path, fs::Permissions::from_mode(0o644));
        }
    }

    Ok(pubkey_path)
}

fn verify_github_ssh() {
    print!(
        "  Press {} once added to GitHub to verify connection (or Enter to skip)... ",
        "[Enter]".bold()
    );
    let _ = io::stdout().flush();
    let mut pause = String::new();
    let _ = io::stdin().lock().read_line(&mut pause);

    let pb = Runner::create_spinner("Verifying GitHub SSH connection...");
    let start_test = std::time::Instant::now();
    let output = std::process::Command::new("ssh")
        .args([
            "-T",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-o",
            "ConnectTimeout=8",
            "git@github.com",
        ])
        .output();
    let test_dur = crate::runner::format_duration(start_test.elapsed());

    pb.finish_and_clear();
    match output {
        Ok(out) => handle_ssh_output(&out.stdout, &out.stderr, &test_dur),
        Err(e) => println!(
            "  {} Failed to execute ssh: {e} {}",
            "⚠".yellow().bold(),
            format!("({test_dur})").dimmed()
        ),
    }
}

fn handle_ssh_output(stdout: &[u8], stderr: &[u8], test_dur: &str) {
    let resp = format!(
        "{}\n{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    );
    if resp.contains("successfully authenticated") {
        let greeting = resp
            .lines()
            .find(|l| l.contains("successfully authenticated"))
            .map_or("Hi! You've successfully authenticated.", str::trim);
        println!(
            "  {} GitHub SSH verified: {} {}",
            "✔".green().bold(),
            greeting.green(),
            format!("({test_dur})").dimmed()
        );
    } else {
        show_ssh_unverified(&resp, test_dur);
    }
}

fn show_ssh_unverified(resp: &str, test_dur: &str) {
    if resp.contains("Permission denied") {
        println!(
            "  {} GitHub SSH not authenticated yet (key not added or pending). {}",
            "⚠".yellow().bold(),
            format!("({test_dur})").dimmed()
        );
        println!(
            "    You can verify later with: {}",
            "ssh -T git@github.com".cyan()
        );
    } else {
        let msg = resp.trim();
        if msg.is_empty() {
            println!(
                "  {} Could not verify GitHub connection. {}",
                "⚠".yellow().bold(),
                format!("({test_dur})").dimmed()
            );
        } else {
            println!(
                "  {} SSH test response: {} {}",
                "⚠".yellow().bold(),
                msg.dimmed(),
                format!("({test_dur})").dimmed()
            );
        }
    }
}
