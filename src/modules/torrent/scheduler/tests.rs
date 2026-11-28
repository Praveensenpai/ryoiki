//! Tests for Seedr single-slot promotion and space guarding.

use super::*;

fn entry(hash: &str, seq: u64, state: QueueState) -> QueueEntry {
    QueueEntry {
        hash: hash.to_string(),
        magnet: format!("magnet:?xt=urn:btih:{hash}"),
        name: hash.to_string(),
        seq,
        state,
        activated_at: None,
        slow_since: None,
    }
}

#[test]
fn test_pick_promotion_idle_when_active() {
    let entries = vec![
        entry("a", 1, QueueState::Active),
        entry("b", 2, QueueState::Queued),
    ];
    assert!(matches!(
        pick_promotion(&entries, QueuePolicy::Fifo, None, None),
        Promotion::Idle
    ));
}

#[test]
fn test_pick_promotion_fifo_and_lifo() {
    let entries = vec![
        entry("a", 1, QueueState::Queued),
        entry("b", 2, QueueState::Queued),
    ];
    let Promotion::Start(f) = pick_promotion(&entries, QueuePolicy::Fifo, None, None) else {
        panic!("expected start");
    };
    assert_eq!(f, "a");
    let Promotion::Start(l) = pick_promotion(&entries, QueuePolicy::Lifo, None, None) else {
        panic!("expected start");
    };
    assert_eq!(l, "b");
}

#[test]
fn test_pick_promotion_space_blocks() {
    let entries = vec![entry("a", 1, QueueState::Queued)];
    assert!(matches!(
        pick_promotion(&entries, QueuePolicy::Fifo, Some(100), Some(500)),
        Promotion::SpaceBlocked { .. }
    ));
}

#[test]
fn test_pick_promotion_allows_when_fits() {
    let entries = vec![entry("a", 1, QueueState::Queued)];
    assert!(matches!(
        pick_promotion(&entries, QueuePolicy::Fifo, Some(500), Some(100)),
        Promotion::Start(_)
    ));
}

#[test]
fn test_pick_promotion_ignores_space_when_unknown() {
    let entries = vec![entry("a", 1, QueueState::Queued)];
    assert!(matches!(
        pick_promotion(&entries, QueuePolicy::Fifo, None, Some(100)),
        Promotion::Start(_)
    ));
}

#[test]
fn test_demote_active_for_priority_requeues_at_front() {
    let mut q = queue::SeedrQueue::default();
    q.enqueue("a", "magnet:?xt=urn:btih:a", "Torrent A");
    assert!(q.activate("a"));
    q.enqueue("b", "magnet:?xt=urn:btih:b", "Torrent B");
    q.enqueue("c", "magnet:?xt=urn:btih:c", "Torrent C");

    let demoted = demote_active_for_priority(&mut q);
    assert_eq!(demoted.as_ref().map(|e| e.hash.as_str()), Some("a"));
    assert!(!q.has_active());

    // Priority torrent B becomes active; previously discarded A is first in queue.
    assert!(q.activate("b"));
    q.finish("b", QueueState::Done);

    let Promotion::Start(next) = pick_promotion(&q.entries, QueuePolicy::Fifo, None, None) else {
        panic!("expected previously discarded torrent A to be promoted next");
    };
    assert_eq!(next, "a");
}
