use crate::tui::RunOptions;
use anyhow::{bail, Context, Result};
use std::process::Command;
use std::time::Duration;

/// Runs a blocking task on a worker thread with a hard wall-clock limit.
///
/// DNS resolution can block past the HTTP client timeout, so the UI must never
/// wait on it forever. The worker is detached; a late result is discarded.
pub(crate) fn with_timeout<T, F>(secs: u64, task: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(task());
    });
    match rx.recv_timeout(Duration::from_secs(secs)) {
        Ok(result) => result,
        Err(_) => bail!("request timed out after {secs}s"),
    }
}

pub(super) fn ensure_sudo() -> Result<()> {
    if unsafe { libc::geteuid() } == 0 {
        return Ok(());
    }
    let cached = Command::new("sudo")
        .args(["-n", "true"])
        .status()
        .is_ok_and(|status| status.success());
    if cached {
        return Ok(());
    }
    println!("  sudo is required; enter your password if prompted.");
    let status = Command::new("sudo")
        .arg("-v")
        .status()
        .context("failed to run sudo")?;
    if !status.success() {
        bail!("sudo authentication failed");
    }
    Ok(())
}

pub(super) fn run_root(program: &str, args: &[&str], opts: RunOptions) -> Result<()> {
    if opts.dry_run {
        println!("  dry-run: {program} {}", args.join(" "));
        return Ok(());
    }
    let is_root = unsafe { libc::geteuid() } == 0;
    if !is_root {
        ensure_sudo()?;
    }
    let output = if is_root {
        Command::new(program).args(args).output()
    } else {
        Command::new("sudo")
            .args(["-n", program])
            .args(args)
            .output()
    }
    .with_context(|| format!("failed to run {program}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("{program} {} failed: {}", args.join(" "), stderr.trim());
    }
    Ok(())
}
