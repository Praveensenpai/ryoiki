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
