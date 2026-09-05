use anyhow::{bail, Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Formats a time duration into a compact, human-readable string.
pub fn format_duration(d: Duration) -> String {
    let millis = d.as_millis();
    if millis == 0 {
        "<1ms".to_string()
    } else if millis < 1000 {
        format!("{millis}ms")
    } else {
        let total_secs = d.as_secs_f64();
        if total_secs < 60.0 {
            format!("{total_secs:.1}s")
        } else {
            let mins = d.as_secs() / 60;
            let secs = d.as_secs() % 60;
            if secs == 0 {
                format!("{mins}m")
            } else {
                format!("{mins}m {secs}s")
            }
        }
    }
}

/// Orchestrates command execution, process tracking, and debug logging.
pub struct Runner {
    pub log_path: PathBuf,
    pub log_file: File,
    pub dry_run: bool,
    pub verbose: bool,
}

impl Runner {
    /// Creates a new Runner instance and initializes the persistent execution log.
    pub fn new(dry_run: bool, verbose: bool) -> Result<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let state_dir = Path::new(&home).join(".local/state/ryoiki");
        fs::create_dir_all(&state_dir)
            .context("Failed to create log directory ~/.local/state/ryoiki")?;

        let log_path = state_dir.join("install.log");
        let log_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)
            .context("Failed to open install.log")?;

        Ok(Self {
            log_path,
            log_file,
            dry_run,
            verbose,
        })
    }

    /// Checks if a given command executable exists in PATH or at an absolute path.
    pub fn command_exists(cmd: &str) -> bool {
        if let Ok(paths) = std::env::var("PATH") {
            for path in std::env::split_paths(&paths) {
                let full = path.join(cmd);
                if full.is_file() {
                    return true;
                }
            }
        }
        Path::new(cmd).exists()
    }

    /// Authenticates sudo privileges upfront if not already cached and keeps them alive in a background thread.
    pub fn ensure_sudo(&self) -> Result<()> {
        if self.dry_run || unsafe { libc::geteuid() } == 0 {
            return Ok(());
        }

        let is_cached = Command::new("sudo")
            .args(["-n", "true"])
            .output()
            .is_ok_and(|o| o.status.success());

        if !is_cached {
            prompt_sudo_auth()?;
        }

        spawn_sudo_keepalive();
        Ok(())
    }

    /// Creates an aesthetic single-line terminal progress spinner with cyan accents.
    pub fn create_spinner(message: &str) -> ProgressBar {
        let pb = ProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(80));
        pb.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner:.cyan} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        pb.set_message(message.to_string());
        pb
    }

    /// Silently executes a subprocess command while displaying a spinner, logging all stdout/stderr.
    pub fn exec_silent(&mut self, desc: &str, program: &str, args: &[&str]) -> Result<()> {
        if self.dry_run {
            println!(
                "  {} [dry-run] {} {}",
                "•".dimmed(),
                program,
                args.join(" ")
            );
            return Ok(());
        }

        let start = std::time::Instant::now();
        let pb = Self::create_spinner(desc);

        let (status, out_buf, err_buf) = run_process(program, args)?;
        let elapsed = format_duration(start.elapsed());

        self.log_output(program, args, &out_buf, &err_buf);

        if self.verbose {
            print!("{}", String::from_utf8_lossy(&out_buf));
            eprint!("{}", String::from_utf8_lossy(&err_buf));
        }

        pb.finish_and_clear();
        self.handle_exec_result(desc, &elapsed, status, &err_buf)
    }

    fn log_output(&mut self, program: &str, args: &[&str], out: &[u8], err: &[u8]) {
        let _ = writeln!(
            self.log_file,
            "\n--- COMMAND: {} {} ---",
            program,
            args.join(" ")
        );
        let _ = self.log_file.write_all(out);
        let _ = self.log_file.write_all(err);
        let _ = self.log_file.flush();
    }

    fn handle_exec_result(
        &self,
        desc: &str,
        elapsed: &str,
        status: std::process::ExitStatus,
        err_buf: &[u8],
    ) -> Result<()> {
        if status.success() {
            println!(
                "  {} {} {}",
                "✔".green().bold(),
                desc,
                format!("({elapsed})").dimmed()
            );
            Ok(())
        } else {
            eprintln!(
                "  {} {} {}",
                "✖".red().bold(),
                desc,
                format!("({elapsed})").dimmed()
            );
            let last_error = String::from_utf8_lossy(err_buf);
            let summary = last_error.lines().rev().take(5).collect::<Vec<_>>();
            if !summary.is_empty() {
                eprintln!("    {}", summary.join("\n    ").dimmed());
            }
            eprintln!(
                "    See full logs: {}",
                self.log_path.display().to_string().cyan()
            );
            bail!("Command failed with exit code: {:?}", status.code());
        }
    }

    /// Executes a bash command string with silent progress feedback and logging.
    pub fn exec_bash(&mut self, desc: &str, script: &str) -> Result<()> {
        self.exec_silent(desc, "bash", &["-c", script])
    }

    /// Installs APT packages non-interactively via sudo apt-get install.
    pub fn apt_install(&mut self, desc: &str, packages: &[&str]) -> Result<()> {
        let mut args = vec!["apt-get", "install", "-y", "--no-install-recommends"];
        args.extend(packages);
        self.exec_silent(desc, "sudo", &args)
    }

    /// Updates APT repository package indices.
    pub fn apt_update(&mut self) -> Result<()> {
        self.exec_silent(
            "Updating APT package lists...",
            "sudo",
            &["apt-get", "update", "-y"],
        )
    }
}

fn prompt_sudo_auth() -> Result<()> {
    println!(
        "  {} Sudo credentials required for server setup.",
        "🔒".bold()
    );
    let status = Command::new("sudo")
        .arg("-v")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Failed to authenticate sudo")?;

    if !status.success() {
        bail!("Sudo authentication failed or was cancelled.");
    }
    println!();
    Ok(())
}

fn spawn_sudo_keepalive() {
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(60));
        let _ = Command::new("sudo")
            .args(["-v"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    });
}

fn run_process(
    program: &str,
    args: &[&str],
) -> Result<(std::process::ExitStatus, Vec<u8>, Vec<u8>)> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn command: {program}"))?;

    let mut stdout = child.stdout.take().context("Failed to capture stdout")?;
    let mut stderr = child.stderr.take().context("Failed to capture stderr")?;

    let mut out_buf = Vec::new();
    let mut err_buf = Vec::new();

    let _ = stdout.read_to_end(&mut out_buf);
    let _ = stderr.read_to_end(&mut err_buf);

    let status = child.wait()?;
    Ok((status, out_buf, err_buf))
}
