use super::super::decisions::DecisionStore;
use super::*;
use std::path::PathBuf;

#[test]
fn test_build_candidates_prefers_recommendation() {
    let probe = MediaProbe {
        audio_languages: vec!["Tamil".to_string(), "Telugu".to_string()],
        ..Default::default()
    };
    let c = build_candidates(Some(&probe), Some("Telugu"));
    assert_eq!(c[0], "Telugu");
    assert!(c.contains(&"Tamil".to_string()));
}

#[test]
fn test_build_candidates_fallback() {
    let c = build_candidates(None, None);
    assert_eq!(c, vec!["Original".to_string()]);
}

#[test]
fn test_render_prompt_marks_recommendation() {
    let entry = PendingDecision {
        id: "abc".to_string(),
        title: "Sample (2024) [Multi].mkv".to_string(),
        stripped_path: PathBuf::from("/a.mkv"),
        multi_path: PathBuf::from("/b.mkv"),
        recommended: Some("Telugu".to_string()),
        candidates: vec!["Telugu".to_string(), "Tamil".to_string()],
        created_secs: now_secs(),
        last_reminded_secs: 0,
        prompt_msg_id: None,
    };
    let card = render_prompt(&entry);
    assert!(card.contains("Telugu"));
    assert!(card.contains("AI recommends"));
}

#[test]
fn test_store_roundtrip_serialization() {
    let store = DecisionStore::default();
    let json = serde_json::to_string(&store).unwrap_or_default();
    assert!(serde_json::from_str::<DecisionStore>(&json).is_ok());
}

#[test]
fn test_should_remove_stripped_never_deletes_multi() {
    let tmp = std::env::temp_dir().join(format!("ryoiki_rm_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let file = tmp.join("Jailer (2023) [Multi] [1080p].mkv");
    std::fs::write(&file, b"x").unwrap_or_default();
    let applied = tmp.join("Jailer (2023) [Tamil] [1080p].mkv");

    // Preserved-multi staging: stripped_path == multi_path == the [Multi].
    assert!(!should_remove_stripped(&file, &file, &applied));
}

#[test]
fn test_should_remove_stripped_removes_superseded_native() {
    let tmp = std::env::temp_dir().join(format!("ryoiki_rm2_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let native = tmp.join("Jailer (2023) [Tamil] [1080p].mkv");
    std::fs::write(&native, b"x").unwrap_or_default();
    let multi = tmp.join("Jailer (2023) [Multi] [1080p].mkv");
    let applied = tmp.join("Jailer (2023) [Telugu] [1080p].mkv");

    // Normal staging: the stale native copy is removed, the multi is not.
    assert!(should_remove_stripped(&native, &multi, &applied));
}
