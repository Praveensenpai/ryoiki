//! Tests for Seedr task polling, live-index matching, and reconciliation.

use super::*;

fn sample(name: &str, pid: Option<u32>) -> SeedrTaskState {
    SeedrTaskState {
        file_name: name.to_string(),
        downloaded_bytes: 0,
        total_bytes: 1024,
        speed_bps: 0,
        eta_seconds: 0,
        status: "Caching".to_string(),
        pid,
    }
}

fn list_with_torrent(name: &str) -> SeedrCloudList {
    SeedrCloudList {
        torrents: vec![SeedrCloudTorrent {
            id: 42,
            name: name.to_string(),
            progress: Some(50.0),
            size: Some(2048),
            download_rate: Some(100),
        }],
        folders: Vec::new(),
        files: Vec::new(),
        space_max: Some(1024),
        space_used: Some(512),
    }
}

#[test]
fn test_live_index_matches_and_normalizes() {
    let index = LiveIndex::from_list(&list_with_torrent("My.Show"));
    assert!(index.contains("my.show"));
    assert!(index.contains("  MY.SHOW "));
    assert!(!index.contains("other"));
}

#[test]
fn test_live_index_matches_folder_prefix() {
    let index = LiveIndex::from_list(&list_with_torrent("Some Release"));
    assert!(index.contains("folder-Some Release"));
}

#[test]
fn test_verdict_keeps_present_task() {
    let index = LiveIndex::from_list(&list_with_torrent("Present"));
    assert!(matches!(
        verdict(&sample("Present", None), &index),
        TaskVerdict::Keep
    ));
}

#[test]
fn test_verdict_prunes_absent_dead_task() {
    let index = LiveIndex::from_list(&list_with_torrent("Other"));
    assert!(matches!(
        verdict(&sample("Gone", None), &index),
        TaskVerdict::Prune
    ));
}

#[test]
fn test_verdict_keeps_absent_but_alive_task() {
    let index = LiveIndex::from_list(&list_with_torrent("Other"));
    let own_pid = std::process::id();
    assert!(matches!(
        verdict(&sample("Gone", Some(own_pid)), &index),
        TaskVerdict::Keep
    ));
}

#[test]
fn test_worker_alive_detects_self() {
    assert!(worker_alive(Some(std::process::id())));
}

#[test]
fn test_synthesize_from_live_maps_fields() {
    let tasks = synthesize_from_live(&list_with_torrent("Cloud Item"));
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].file_name, "Cloud Item");
    assert_eq!(tasks[0].total_bytes, 2048);
    assert_eq!(tasks[0].downloaded_bytes, 1024);
}
