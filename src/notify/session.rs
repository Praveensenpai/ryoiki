//! Per-IP login alert window and audit log.
//!
//! The root (PAM) hook and the unprivileged (shell profile) hook both feed this
//! module, so state lives in one sticky directory that survives reboots. A
//! per-IP marker stores the epoch seconds of the last *delivered* alert: while
//! it is younger than the configured window, repeat logins are suppressed and
//! only appended to the audit log. The marker is written after a successful
//! send, so a failed Telegram call never silences an IP for the whole window.

use std::ffi::CString;
use std::fs;
use std::io::Write as _;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Shared system-wide state root, writable by every login hook.
const SYSTEM_STATE_DIR: &str = "/var/lib/ryoiki";
/// Overrides state discovery (tests, sandboxed installs).
const ENV_STATE_DIR: &str = "RYOIKI_STATE_DIR";
const SESSIONS_DIR: &str = "sessions";
const LOG_FILE: &str = "login.log";
/// Markers written by the retired `/tmp`-based implementation, adopted once.
const LEGACY_PREFIX: &str = "ryoiki_session_";
const DIR_MODE: libc::mode_t = 0o1777;
const FILE_MODE: libc::mode_t = 0o666;

/// Result recorded for one login attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginOutcome {
    /// Alert delivered to Telegram.
    Alerted,
    /// This IP already alerted inside the window.
    Suppressed,
    /// Telegram send failed; the window was not advanced.
    SendFailed,
}

impl LoginOutcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Alerted => "alerted",
            Self::Suppressed => "suppressed",
            Self::SendFailed => "send-failed",
        }
    }
}

/// Creates the sticky shared state root used when hooks run as root.
#[must_use]
pub fn ensure_shared_state_dir() -> bool {
    prepare(Path::new(SYSTEM_STATE_DIR), true).is_ok()
}

/// True when `ip` already alerted inside `window_mins` minutes.
#[must_use]
pub fn is_session_suppressed(ip: &str, window_mins: u64) -> bool {
    is_suppressed_in(&resolve_state_dir(), ip, window_mins)
}

/// Stamps now as the last delivered alert for `ip`.
pub fn record_session_login(ip: &str) {
    record_in(&resolve_state_dir(), ip);
}

/// Appends one login to the audit log, whether alerted or suppressed.
pub fn log_login(user: &str, ip: &str, service: &str, tty: &str, outcome: LoginOutcome) {
    log_in(&resolve_state_dir(), user, ip, service, tty, outcome);
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

fn is_suppressed_in(dir: &Path, ip: &str, window_mins: u64) -> bool {
    if window_mins == 0 {
        return false;
    }
    let window_secs = window_mins.saturating_mul(60);
    marker_last_alert(dir, ip).is_some_and(|last| now_secs().saturating_sub(last) < window_secs)
}

fn record_in(dir: &Path, ip: &str) {
    let path = marker_path(dir, ip);
    let Ok(mut file) = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
    else {
        return;
    };
    let _ = writeln!(file, "{}", now_secs());
    set_mode(&path, FILE_MODE);
}

fn log_in(dir: &Path, user: &str, ip: &str, service: &str, tty: &str, outcome: LoginOutcome) {
    let path = dir.join(LOG_FILE);
    let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    set_mode(&path, FILE_MODE);
    let _ = writeln!(
        file,
        "{}\t{}\t{}\t{}\t{}\t{}",
        crate::notify::client::ist_timestamp(),
        outcome.as_str(),
        user,
        ip,
        service,
        tty
    );
}

fn marker_path(dir: &Path, ip: &str) -> PathBuf {
    dir.join(SESSIONS_DIR).join(sanitize_identifier(ip))
}

/// Reads the newest known alert timestamp for `ip`, falling back to the legacy
/// `/tmp` marker so the first run after upgrade does not re-alert.
fn marker_last_alert(dir: &Path, ip: &str) -> Option<u64> {
    if let Ok(raw) = fs::read_to_string(marker_path(dir, ip)) {
        if let Ok(secs) = raw.trim().parse::<u64>() {
            return Some(secs);
        }
    }
    legacy_marker_secs(ip)
}

fn legacy_marker_secs(ip: &str) -> Option<u64> {
    // SAFETY: getuid takes no arguments and cannot fail.
    let uid = unsafe { libc::getuid() };
    let name = format!("{LEGACY_PREFIX}{uid}_{}", sanitize_identifier(ip));
    let modified = fs::metadata(std::env::temp_dir().join(name))
        .ok()?
        .modified()
        .ok()?;
    Some(modified.duration_since(UNIX_EPOCH).ok()?.as_secs())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn sanitize_identifier(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
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
    fn test_sanitize_identifier() {
        assert_eq!(sanitize_identifier("192.168.1.1"), "192_168_1_1");
        assert_eq!(sanitize_identifier("2001:db8::1"), "2001_db8__1");
        assert_eq!(sanitize_identifier("Local Console"), "Local_Console");
    }

    #[test]
    fn test_zero_window_never_suppresses() {
        let dir = temp_state("zero");
        record_in(&dir, "1.2.3.4");
        assert!(!is_suppressed_in(&dir, "1.2.3.4", 0));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_alert_suppresses_within_window() {
        let dir = temp_state("within");
        record_in(&dir, "198.51.100.99");
        assert!(is_suppressed_in(&dir, "198.51.100.99", 10_080));
        assert!(!is_suppressed_in(&dir, "198.51.100.100", 10_080));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_expired_window_alerts_again() {
        let dir = temp_state("expired");
        let stale = now_secs().saturating_sub(8 * 24 * 3600);
        let _ = fs::write(marker_path(&dir, "203.0.113.5"), stale.to_string());
        assert!(!is_suppressed_in(&dir, "203.0.113.5", 10_080));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_log_login_appends_every_outcome() {
        let dir = temp_state("log");
        log_in(
            &dir,
            "neko",
            "203.0.113.9",
            "sshd",
            "pts/1",
            LoginOutcome::Suppressed,
        );
        log_in(
            &dir,
            "neko",
            "203.0.113.9",
            "sshd",
            "pts/1",
            LoginOutcome::Alerted,
        );
        let content = fs::read_to_string(dir.join(LOG_FILE)).unwrap_or_default();
        assert_eq!(content.lines().count(), 2, "log: {content}");
        assert!(content.contains("suppressed"));
        assert!(content.contains("alerted"));
        let _ = fs::remove_dir_all(dir);
    }
}
