use std::fs;
use std::time::Duration;

#[must_use]
pub fn is_session_suppressed(ip: &str, cooldown_mins: u64) -> bool {
    if cooldown_mins == 0 {
        return false;
    }

    let sanitized = sanitize_identifier(ip);
    let target_suffix = format!("_{sanitized}");
    let cooldown_duration = Duration::from_secs(cooldown_mins.saturating_mul(60));

    let Ok(entries) = fs::read_dir(std::env::temp_dir()) else {
        return false;
    };

    let mut suppressed = false;
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(ToString::to_string) else {
            continue;
        };

        if !name.starts_with("ryoiki_session_") || !name.ends_with(&target_suffix) {
            continue;
        }

        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                if let Ok(elapsed) = modified.elapsed() {
                    if elapsed < cooldown_duration {
                        suppressed = true;
                    } else {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }

    suppressed
}

pub fn record_session_login(ip: &str) {
    let sanitized = sanitize_identifier(ip);
    let uid = unsafe { libc::getuid() };
    let filename = format!("ryoiki_session_{uid}_{sanitized}");
    let path = std::env::temp_dir().join(filename);
    let _ = fs::write(path, b"1");
}

fn sanitize_identifier(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_identifier() {
        assert_eq!(sanitize_identifier("192.168.1.1"), "192_168_1_1");
        assert_eq!(sanitize_identifier("2001:db8::1"), "2001_db8__1");
        assert_eq!(sanitize_identifier("Local Console"), "Local_Console");
    }

    #[test]
    fn test_is_session_suppressed_zero_cooldown() {
        assert!(!is_session_suppressed("1.2.3.4", 0));
    }

    #[test]
    fn test_record_and_suppress_session() {
        let test_ip = "198.51.100.99";
        record_session_login(test_ip);
        assert!(is_session_suppressed(test_ip, 60));

        let sanitized = sanitize_identifier(test_ip);
        let uid = unsafe { libc::getuid() };
        let filename = format!("ryoiki_session_{uid}_{sanitized}");
        let _ = fs::remove_file(std::env::temp_dir().join(filename));
    }
}
