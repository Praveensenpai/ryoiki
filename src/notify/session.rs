//! Per-IP login audit log.
//!
//! Logs every login attempt to a persistent audit file. No Telegram
//! notifications are sent; the cooldown window and per-IP markers have been
//! removed per the "logging only" requirement.

use std::ffi::CString;
use std::fs;
use std::io::Write as _;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// Shared system-wide state root, writable by every login hook.
const SYSTEM_STATE_DIR: &str = "/var/lib/ryoiki";
/// Overrides state discovery (tests, sandboxed installs).
const ENV_STATE_DIR: &str = "RYOIKI_STATE_DIR";
const SESSIONS_DIR: &str = "sessions";
const LOG_FILE: &str = "login.log";
const DIR_MODE: libc::mode_t = 0o1777;
const FILE_MODE: libc::mode_t = 0o666;

/// Result recorded for one login attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginOutcome {
    /// Login was logged (Telegram notification disabled).
    Alerted,
}

impl LoginOutcome {
    const fn as_str() -> &'static str {
        "alerted"
    }
}

/// Creates the sticky shared state root used when hooks run as root.
#[must_use]
pub fn ensure_shared_state_dir() -> bool {
    prepare(Path::new(SYSTEM_STATE_DIR), true).is_ok()
}

/// Appends one login to the audit log.
pub fn log_login(user: &str, ip: &str, service: &str, tty: &str, _outcome: LoginOutcome) {
    let dir = resolve_state_dir();
    let log_path = dir.join(LOG_FILE);
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        set_mode(&log_path, FILE_MODE);
        let _ = writeln!(
            file,
            "{}\t{}\t{}\t{}\t{}\t{}",
            crate::notify::client::ist_timestamp(),
            LoginOutcome::as_str(),
            user,
            ip,
            service,
            tty
        );
    }
}

fn resolve_state_dir() -> PathBuf {
    if let Some(dir) = env_state_dir() {
        let _ = prepare(&dir, false);
        return dir;
    }
    let system = PathBuf::from(SYSTEM_STATE_DIR);
    if prepare(&system, true).is_ok() {
        return system;
    }
    let user = xdg_state_base().join("ryoiki");
    let _ = prepare(&user, false);
    user
}

fn env_state_dir() -> Option<PathBuf> {
    non_empty_env(ENV_STATE_DIR).map(PathBuf::from)
}

fn xdg_state_base() -> PathBuf {
    non_empty_env("XDG_STATE_HOME").map_or_else(|| home_dir().join(".local/state"), PathBuf::from)
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

fn home_dir() -> PathBuf {
    std::env::var("HOME").map_or_else(|_| PathBuf::from("/root"), PathBuf::from)
}

fn prepare(dir: &Path, shared: bool) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    if shared {
        set_mode(dir, DIR_MODE);
    }
    fs::create_dir_all(dir.join(SESSIONS_DIR))
}

fn set_mode(path: &Path, mode: libc::mode_t) {
    let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) else {
        return;
    };
    // SAFETY: c_path is a valid NUL-terminated string for the call's duration.
    unsafe {
        libc::chmod(c_path.as_ptr(), mode);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_state(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let dir =
            std::env::temp_dir().join(format!("ryoiki-state-{tag}-{}-{nanos}", std::process::id()));
        let _ = prepare(&dir, false);
        dir
    }

    #[test]
    fn test_log_login_appends_entry() {
        let dir = temp_state("log");
        std::env::set_var("RYOIKI_STATE_DIR", dir.to_string_lossy().as_ref());
        log_login(
            "neko",
            "203.0.113.9",
            "sshd",
            "pts/1",
            LoginOutcome::Alerted,
        );
        let content = fs::read_to_string(dir.join(LOG_FILE)).unwrap_or_default();
        // Log format: timestamp\toutcome\tuser\tip\tservice\tty
        assert!(
            content.contains("neko"),
            "expected 'neko' in log, got: {content:?}"
        );
        assert!(
            content.contains("203.0.113.9"),
            "expected IP in log, got: {content:?}"
        );
        let _ = fs::remove_dir_all(dir);
    }
}
