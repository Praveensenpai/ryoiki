//! Registry and timeout for interactive Seedr queue prompts.
//!
//! When a magnet arrives while the Seedr slot is busy, the bot asks the user
//! whether to keep it queued or jump the line. The default (queued) is already
//! applied before asking, so an unanswered prompt simply resolves to it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Message shown when the prompt times out with no user response.
pub const AUTO_QUEUED_TEXT: &str = "⌛ <b>Auto-selected: keep queued</b>\n\
    <i>No response within the timeout — this magnet stays in the Seedr queue \
    and starts automatically when the slot frees.</i>";

/// A prompt awaiting a user decision.
#[derive(Debug, Clone, Copy)]
pub struct PendingPrompt {
    pub msg_id: i64,
}

/// Shared map of `hash -> pending prompt`.
pub type Prompts = Arc<Mutex<HashMap<String, PendingPrompt>>>;

/// Creates an empty prompt registry.
#[must_use]
pub fn new_registry() -> Prompts {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Records a prompt for a hash.
pub fn register(prompts: &Prompts, hash: &str, msg_id: i64) {
    if let Ok(mut map) = prompts.lock() {
        map.insert(hash.to_string(), PendingPrompt { msg_id });
    }
}

/// Removes and returns a pending prompt, if one exists.
#[must_use]
pub fn take(prompts: &Prompts, hash: &str) -> Option<PendingPrompt> {
    prompts.lock().ok().and_then(|mut map| map.remove(hash))
}

/// Resolves an unanswered prompt after `timeout_secs`.
///
/// If the user already answered, [`take`] returns `None` and `on_timeout` is
/// never called. Otherwise `on_timeout` receives the original message id so the
/// caller can edit it to its default state.
pub fn spawn_timeout<F>(prompts: &Prompts, hash: &str, timeout_secs: u64, on_timeout: F)
where
    F: FnOnce(i64) + Send + 'static,
{
    let prompts = Arc::clone(prompts);
    let hash = hash.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(timeout_secs));
        if let Some(pending) = take(&prompts, &hash) {
            on_timeout(pending.msg_id);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn test_register_then_take() {
        let prompts = new_registry();
        register(&prompts, "abc", 42);
        let taken = take(&prompts, "abc");
        assert_eq!(taken.map(|p| p.msg_id), Some(42));
        assert!(take(&prompts, "abc").is_none());
    }

    #[test]
    fn test_take_missing_returns_none() {
        let prompts = new_registry();
        assert!(take(&prompts, "nope").is_none());
    }

    #[test]
    fn test_timeout_fires_and_resolves_pending() {
        let prompts = new_registry();
        register(&prompts, "h1", 77);
        let (tx, rx) = mpsc::channel();
        spawn_timeout(&prompts, "h1", 1, move |id| {
            let _ = tx.send(id);
        });
        let fired = rx.recv_timeout(Duration::from_secs(5));
        assert_eq!(fired.ok(), Some(77), "timeout must resolve with the msg id");
        assert!(take(&prompts, "h1").is_none(), "pending prompt is consumed");
    }

    #[test]
    fn test_timeout_skips_already_answered() {
        let prompts = new_registry();
        register(&prompts, "h2", 88);
        assert!(take(&prompts, "h2").is_some(), "user answered first");
        let (tx, rx) = mpsc::channel();
        spawn_timeout(&prompts, "h2", 1, move |id| {
            let _ = tx.send(id);
        });
        assert!(
            rx.recv_timeout(Duration::from_secs(3)).is_err(),
            "callback must not fire after the prompt was answered"
        );
    }
}
