//! Tests for the persisted Seedr queue and its locking.

use super::*;
use std::sync::Arc;
use std::thread;

fn queue_with(hashes: &[&str]) -> SeedrQueue {
    let mut q = SeedrQueue::default();
    for h in hashes {
        q.enqueue(h, &format!("magnet:?xt=urn:btih:{h}"), h);
    }
    q
}

fn temp_path(tag: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    std::env::temp_dir().join(format!("ryoiki-queue-{tag}-{pid}-{nanos}"))
}

#[test]
fn test_enqueue_assigns_increasing_seq() {
    let q = queue_with(&["a", "b", "c"]);
    assert_eq!(q.entries.len(), 3);
    assert_eq!(q.entries[0].seq, 1);
    assert_eq!(q.entries[2].seq, 3);
}

#[test]
fn test_enqueue_deduplicates() {
    let mut q = queue_with(&["a"]);
    q.enqueue("a", "magnet:a", "a");
    assert_eq!(q.entries.len(), 1);
}

#[test]
fn test_fifo_picks_oldest() {
    let q = queue_with(&["a", "b", "c"]);
    let mut queued: Vec<&QueueEntry> = q
        .entries
        .iter()
        .filter(|e| e.state == QueueState::Queued)
        .collect();
    queued.sort_by_key(|e| e.seq);
    assert_eq!(queued.first().map(|e| e.hash.as_str()), Some("a"));
}

#[test]
fn test_lifo_picks_newest() {
    let q = queue_with(&["a", "b", "c"]);
    let mut queued: Vec<&QueueEntry> = q
        .entries
        .iter()
        .filter(|e| e.state == QueueState::Queued)
        .collect();
    queued.sort_by_key(|e| e.seq);
    assert_eq!(queued.last().map(|e| e.hash.as_str()), Some("c"));
}

#[test]
fn test_single_active_slot_enforced() {
    let mut q = queue_with(&["a", "b"]);
    assert!(q.activate("a"));
    assert!(q.has_active());
    assert!(!q.activate("b"));
    let active = q.entries.iter().find(|e| e.state == QueueState::Active);
    assert_eq!(active.map(|e| e.hash.as_str()), Some("a"));
}

#[test]
fn test_promote_front_reorders() {
    let mut q = queue_with(&["a", "b", "c"]);
    assert!(q.promote_front("c"));
    let mut queued: Vec<&QueueEntry> = q
        .entries
        .iter()
        .filter(|e| e.state == QueueState::Queued)
        .collect();
    queued.sort_by_key(|e| e.seq);
    assert_eq!(queued.first().map(|e| e.hash.as_str()), Some("c"));
}

#[test]
fn test_position_counts_only_queued() {
    let mut q = queue_with(&["a", "b", "c"]);
    assert!(q.activate("a"));
    assert_eq!(q.position("b"), 1);
    assert_eq!(q.position("c"), 2);
    assert_eq!(q.position("a"), 0);
}

#[test]
fn test_finish_frees_slot_for_next() {
    let mut q = queue_with(&["a", "b"]);
    assert!(q.activate("a"));
    assert!(q.finish("a", QueueState::Done));
    assert!(!q.has_active());
    assert_eq!(q.position("b"), 1);
}

#[test]
fn test_remove_and_roundtrip() {
    let mut q = queue_with(&["a", "b"]);
    assert!(q.remove("a"));
    assert!(!q.remove("a"));
    let json = serde_json::to_string(&q).unwrap_or_default();
    let back: SeedrQueue = serde_json::from_str(&json).unwrap_or_default();
    assert_eq!(back.entries.len(), 1);
    assert_eq!(back.pending_count(), 1);
}

#[test]
fn test_save_load_at_roundtrip() {
    let path = temp_path("roundtrip");
    let mut q = queue_with(&["x", "y"]);
    assert!(q.activate("x"));
    assert!(save_at(&path, &q).is_ok());
    let back = load_at(&path);
    assert_eq!(back.entries.len(), 2);
    assert!(back.has_active());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_concurrent_mutations_do_not_lose_updates() {
    let path = Arc::new(temp_path("concurrent"));
    let workers = 16;
    let handles: Vec<_> = (0..workers)
        .map(|i| {
            let path = Arc::clone(&path);
            thread::spawn(move || {
                with_lock_at(&path, || {
                    let mut q = load_at(&path);
                    q.enqueue(&format!("h{i}"), "magnet", &format!("n{i}"));
                    let _ = save_at(&path, &q);
                });
            })
        })
        .collect();
    for handle in handles {
        assert!(handle.join().is_ok());
    }
    let final_q = load_at(&path);
    assert_eq!(
        final_q.entries.len(),
        workers,
        "every enqueue must survive concurrent read-modify-write"
    );
    let _ = fs::remove_file(&*path);
    let _ = fs::remove_file(lock_path_for(&path));
}

#[test]
fn test_concurrent_activation_keeps_single_slot() {
    let path = Arc::new(temp_path("single-slot"));
    let mut seed = SeedrQueue::default();
    for i in 0..8 {
        seed.enqueue(&format!("s{i}"), "magnet", &format!("s{i}"));
    }
    assert!(save_at(&path, &seed).is_ok());

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let path = Arc::clone(&path);
            thread::spawn(move || {
                with_lock_at(&path, || {
                    let mut q = load_at(&path);
                    if q.has_active() {
                        return;
                    }
                    let hash = format!("s{i}");
                    if q.activate(&hash) {
                        let _ = save_at(&path, &q);
                    }
                });
            })
        })
        .collect();
    for handle in handles {
        assert!(handle.join().is_ok());
    }
    let active = load_at(&path)
        .entries
        .iter()
        .filter(|e| e.state == QueueState::Active)
        .count();
    assert_eq!(
        active, 1,
        "the single Seedr slot must never double-activate"
    );
    let _ = fs::remove_file(&*path);
    let _ = fs::remove_file(lock_path_for(&path));
}
