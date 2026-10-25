//! Owner-managed allowlist for non-owner Telegram users.
//!
//! The owner (`config.chat_id`) always has full access. Everyone else starts
//! locked out: the only command they may send is `/request`, which forwards an
//! Approve/Deny prompt to the owner. Approving persists the user id so the
//! decision survives bot restarts.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Callback prefix used by the owner-only Approve/Deny buttons.
pub const CALLBACK_PREFIX: &str = "cb:access:";

/// On-disk shape of the allowlist.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Allowlist {
    #[serde(default)]
    authorized: Vec<i64>,
}

#[derive(Debug)]
struct Inner {
    authorized: BTreeSet<i64>,
    pending: BTreeSet<i64>,
    path: PathBuf,
}

/// Shared, thread-safe access gate.
#[derive(Clone, Debug)]
pub struct AccessControl(Arc<Mutex<Inner>>);

impl AccessControl {
    /// Loads the allowlist from the XDG state directory.
    ///
    /// A missing or corrupt file yields an empty allowlist: failures here must
    /// never grant access.
    #[must_use]
    pub fn load() -> Self {
        let path = state_path();
        let authorized = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Allowlist>(&raw).ok())
            .map(|list| list.authorized.into_iter().collect())
            .unwrap_or_default();
        Self(Arc::new(Mutex::new(Inner {
            authorized,
            pending: BTreeSet::new(),
            path,
        })))
    }

    /// Builds an in-memory gate backed by `path` (used by tests).
    #[cfg(test)]
    #[must_use]
    pub fn with_path(path: PathBuf) -> Self {
        Self(Arc::new(Mutex::new(Inner {
            authorized: BTreeSet::new(),
            pending: BTreeSet::new(),
            path,
        })))
    }

    /// Returns true when `user_id` already holds granted access.
    #[must_use]
    pub fn is_authorized(&self, user_id: i64) -> bool {
        self.0
            .lock()
            .is_ok_and(|inner| inner.authorized.contains(&user_id))
    }

    /// Returns true when an approval prompt for `user_id` is still unanswered.
    #[must_use]
    pub fn is_pending(&self, user_id: i64) -> bool {
        self.0
            .lock()
            .is_ok_and(|inner| inner.pending.contains(&user_id))
    }

    /// Marks `user_id` as awaiting owner approval.
    ///
    /// Returns `false` when a request was already pending, which keeps repeated
    /// `/request` messages from spamming the owner.
    pub fn mark_pending(&self, user_id: i64) -> bool {
        self.0
            .lock()
            .is_ok_and(|mut inner| inner.pending.insert(user_id))
    }

    /// Clears a pending request without granting access.
    pub fn clear_pending(&self, user_id: i64) {
        if let Ok(mut inner) = self.0.lock() {
            inner.pending.remove(&user_id);
        }
    }

    /// Grants access and persists the allowlist.
    ///
    /// # Errors
    /// Fails when the state file cannot be written, so the caller never claims
    /// success for a grant that would vanish on restart.
    pub fn approve(&self, user_id: i64) -> Result<()> {
        let mut inner = self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("access lock poisoned"))?;
        inner.pending.remove(&user_id);
        inner.authorized.insert(user_id);
        let list = Allowlist {
            authorized: inner.authorized.iter().copied().collect(),
        };
        let json = serde_json::to_string_pretty(&list).context("serialize allowlist")?;
        if let Some(parent) = inner.path.parent() {
            fs::create_dir_all(parent).context("create state dir")?;
        }
        fs::write(&inner.path, json).context("write allowlist")?;
        Ok(())
    }
}

/// `$XDG_STATE_HOME/ryoiki/authorized_users.json` (defaults to `~/.local/state`).
#[must_use]
pub fn state_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_STATE_HOME") {
        return Path::new(&dir).join("ryoiki/authorized_users.json");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    Path::new(&home).join(".local/state/ryoiki/authorized_users.json")
}

/// Parses `cb:access:<ok|no>:<user_id>` into the action and target id.
#[must_use]
pub fn parse_callback(data: &str) -> Option<(AccessAction, i64)> {
    let rest = data.strip_prefix(CALLBACK_PREFIX)?;
    let (action, id) = rest.split_once(':')?;
    let user_id = id.parse::<i64>().ok()?;
    let action = match action {
        "ok" => AccessAction::Approve,
        "no" => AccessAction::Deny,
        _ => return None,
    };
    Some((action, user_id))
}

/// Owner decision carried by an access callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessAction {
    Approve,
    Deny,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        std::env::temp_dir().join(format!(
            "ryoiki-access-{tag}-{}-{nanos}.json",
            std::process::id()
        ))
    }

    #[test]
    fn test_approve_persists_across_reload() -> Result<()> {
        let path = temp_path("persist");
        let gate = AccessControl::with_path(path.clone());
        assert!(!gate.is_authorized(42));
        gate.approve(42)?;
        assert!(gate.is_authorized(42));

        let raw = fs::read_to_string(&path)?;
        let list: Allowlist = serde_json::from_str(&raw)?;
        assert_eq!(list.authorized, vec![42]);
        let _ = fs::remove_file(&path);
        Ok(())
    }

    #[test]
    fn test_mark_pending_is_idempotent() {
        let gate = AccessControl::with_path(temp_path("pending"));
        assert!(gate.mark_pending(7));
        assert!(gate.is_pending(7));
        assert!(!gate.mark_pending(7), "second request must not re-notify");
        gate.clear_pending(7);
        assert!(!gate.is_pending(7));
        assert!(gate.mark_pending(7));
    }

    #[test]
    fn test_callback_parsing() {
        assert_eq!(
            parse_callback("cb:access:ok:123"),
            Some((AccessAction::Approve, 123))
        );
        assert_eq!(
            parse_callback("cb:access:no:456"),
            Some((AccessAction::Deny, 456))
        );
        assert_eq!(parse_callback("cb:access:ok:notanid"), None);
        assert_eq!(parse_callback("cb:access:maybe:1"), None);
        assert_eq!(parse_callback("cb:status"), None);
    }

    #[test]
    fn test_load_missing_file_starts_empty() {
        let gate = AccessControl::with_path(temp_path("missing"));
        assert!(!gate.is_authorized(1));
        assert!(!gate.is_pending(1));
    }
}
