//! Exclusive PID lock for the battery-watch daemon.
//!
//! systemd stops the service with SIGTERM, which terminates the process without
//! running destructors, so a lock removed only in `Drop` survives shutdown and
//! then blocks every restart with "another instance is already running". Clear a
//! lock whose recorded PID is no longer alive before acquiring.

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Default lock path, under `/run` so a reboot clears it.
const PID_FILE: &str = "/run/ryoiki-battery-watch.pid";

/// Acquires the daemon lock at [`PID_FILE`].
pub(super) fn acquire_pid_lock() -> Result<PidGuard> {
    acquire_at(Path::new(PID_FILE))
}

fn acquire_at(pid_path: &Path) -> Result<PidGuard> {
    if pid_path.exists() && !holder_is_alive(pid_path) {
        let _ = fs::remove_file(pid_path);
    }

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(pid_path)
        .map_err(|_| {
            anyhow!(
                "Another battery-watch instance is already running (lock: {PID_FILE}). Exiting."
            )
        })?;

    writeln!(file, "{}", std::process::id()).context("Failed to write PID lock")?;

    Ok(PidGuard {
        path: pid_path.to_path_buf(),
    })
}

/// True when the lock records a PID that is still running.
fn holder_is_alive(pid_path: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(pid_path) else {
        return false;
    };
    let Ok(pid) = contents.trim().parse::<u32>() else {
        return false;
    };
    pid != 0 && pid != std::process::id() && Path::new(&format!("/proc/{pid}")).exists()
}

/// Removes the lock file on drop.
pub(super) struct PidGuard {
    path: PathBuf,
}

impl Drop for PidGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_lock(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        std::env::temp_dir().join(format!("ryoiki-lock-{tag}-{}-{nanos}", std::process::id()))
    }

    #[test]
    fn acquires_when_no_lock_exists() -> Result<()> {
        let path = temp_lock("free");
        let guard = acquire_at(&path)?;
        assert!(path.exists(), "lock must be created");
        drop(guard);
        assert!(!path.exists(), "lock must be removed on drop");
        Ok(())
    }

    #[test]
    fn clears_stale_lock_from_dead_process() -> Result<()> {
        let path = temp_lock("stale");
        fs::write(&path, "99999999")?;
        let guard = acquire_at(&path)?;
        let recorded = fs::read_to_string(&path)?;
        assert_eq!(recorded.trim().parse::<u32>()?, std::process::id());
        drop(guard);
        Ok(())
    }

    #[test]
    fn refuses_when_lock_holder_is_alive() -> Result<()> {
        let path = temp_lock("alive");
        fs::write(&path, "1")?;
        let err = acquire_at(&path);
        assert!(err.is_err(), "live holder must block a second instance");
        let _ = fs::remove_file(&path);
        Ok(())
    }
}
