use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use anyhow::Result;
use colored::*;
use crate::runner::Runner;

pub fn setup(runner: &mut Runner, non_interactive: bool) -> Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let ssh_dir = Path::new(&home).join(".ssh");
    let key_path = ssh_dir.join("id_ed25519");
    let pubkey_path = ssh_dir.join("id_ed25519.pub");

    // 1. Git Identity
    let current_user = std::process::Command::new("git")
        .args(["config", "--global", "user.name"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    let current_email = std::process::Command::new("git")
        .args(["config", "--global", "user.email"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    let (git_user, git_email) = if non_interactive || runner.dry_run {
        let user = if current_user.is_empty() { "Praveensenpai".to_string() } else { current_user };
        let email = if current_email.is_empty() { "pvnt20@gmail.com".to_string() } else { current_email };
        (user, email)
    } else {
        let default_user = if current_user.is_empty() { "Praveensenpai" } else { &current_user };
        let default_email = if current_email.is_empty() { "pvnt20@gmail.com" } else { &current_email };

        print!("  Enter Git username [{}]: ", default_user.cyan());
        io::stdout().flush()?;
        let mut input_user = String::new();
        io::stdin().lock().read_line(&mut input_user)?;
        let input_user = input_user.trim();
        let final_user = if input_user.is_empty() { default_user } else { input_user };

        print!("  Enter Git email [{}]: ", default_email.cyan());
        io::stdout().flush()?;
        let mut input_email = String::new();
        io::stdin().lock().read_line(&mut input_email)?;
        let input_email = input_email.trim();
        let final_email = if input_email.is_empty() { default_email } else { input_email };

        (final_user.to_string(), final_email.to_string())
    };

    runner.exec_silent(
        &format!("Configuring Git identity: {git_user} <{git_email}>"),
        "git",
        &["config", "--global", "user.name", &git_user],
    )?;
    runner.exec_silent(
        "Setting Git global email",
        "git",
        &["config", "--global", "user.email", &git_email],
    )?;
    runner.exec_silent(
        "Setting Git default branch to main",
        "git",
        &["config", "--global", "init.defaultBranch", "main"],
    )?;

    // 2. SSH Key
    if pubkey_path.exists() {
        println!("  {} Existing SSH key found: {}", "✔".green(), pubkey_path.display().to_string().dimmed());
    } else {
        fs::create_dir_all(&ssh_dir)?;
        runner.exec_silent(
            "Generating Ed25519 SSH Key...",
            "ssh-keygen",
            &["-t", "ed25519", "-C", &git_email, "-f", key_path.to_str().unwrap(), "-N", ""],
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

    if let Ok(pubkey) = fs::read_to_string(&pubkey_path) {
        println!("\n  {}", "─── Your GitHub Public SSH Key ───".dimmed());
        println!("  {}", pubkey.trim().cyan().bold());
        println!("  {}", "───────────────────────────────────".dimmed());
        println!("  Paste into: {}\n", "https://github.com/settings/ssh/new".underline().blue());
    }

    if !non_interactive && !runner.dry_run {
        print!("  Press {} once added to GitHub to verify connection (or Enter to skip)... ", "[Enter]".bold());
        io::stdout().flush()?;
        let mut pause = String::new();
        io::stdin().lock().read_line(&mut pause)?;

        let pb = runner.create_spinner("Verifying GitHub SSH connection...");
        let start_test = std::time::Instant::now();
        let output = std::process::Command::new("ssh")
            .args(["-T", "-o", "StrictHostKeyChecking=accept-new", "-o", "ConnectTimeout=8", "git@github.com"])
            .output();
        let test_dur = crate::runner::format_duration(start_test.elapsed());

        match output {
            Ok(out) => {
                let resp = format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                pb.finish_and_clear();
                if resp.contains("successfully authenticated") {
                    let greeting = resp
                        .lines()
                        .find(|l| l.contains("successfully authenticated"))
                        .map(|l| l.trim())
                        .unwrap_or("Hi! You've successfully authenticated.");
                    println!("  {} GitHub SSH verified: {} {}", "✔".green().bold(), greeting.green(), format!("({})", test_dur).dimmed());
                } else if resp.contains("Permission denied") {
                    println!("  {} GitHub SSH not authenticated yet (key not added or pending). {}", "⚠".yellow().bold(), format!("({})", test_dur).dimmed());
                    println!("    You can verify later with: {}", "ssh -T git@github.com".cyan());
                } else {
                    let msg = resp.trim();
                    if msg.is_empty() {
                        println!("  {} Could not verify GitHub connection. {}", "⚠".yellow().bold(), format!("({})", test_dur).dimmed());
                    } else {
                        println!("  {} SSH test response: {} {}", "⚠".yellow().bold(), msg.dimmed(), format!("({})", test_dur).dimmed());
                    }
                }
            }
            Err(e) => {
                pb.finish_and_clear();
                println!("  {} Failed to execute ssh: {} {}", "⚠".yellow().bold(), e, format!("({})", test_dur).dimmed());
            }
        }
    }

    Ok(())
}
