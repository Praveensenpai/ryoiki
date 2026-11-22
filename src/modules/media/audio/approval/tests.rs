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
    assert!(card.contains("\n<code>AI RECOMMENDS"));
    assert!(card.contains("\n<code>TRACKS"));
}

#[test]
fn test_register_rejects_single_distinct_language() {
    // Two streams tagged Kannada (5.1 + stereo) offer no language choice, so
    // registration must not create an unanswerable pending entry.
    let probe = MediaProbe {
        audio_stream_count: 2,
        audio_languages: vec!["Kannada".to_string()],
        ..Default::default()
    };
    assert!(!probe::needs_language_decision(&probe));

    // Untagged streams still prompt: the language is unknown, not absent.
    let untagged = MediaProbe {
        audio_stream_count: 2,
        audio_languages: Vec::new(),
        ..Default::default()
    };
    assert!(probe::needs_language_decision(&untagged));
}

#[test]
fn test_language_keyboard_has_silence_row() {
    let kb = crate::bot::keyboards::language_keyboard(
        "abc",
        &["Tamil".to_string(), "Telugu".to_string()],
        Some("Telugu"),
    );
    assert_eq!(kb.inline_keyboard.len(), 3);
    let silence_row = kb.inline_keyboard.last().and_then(|r| r.first());
    assert!(
        silence_row.is_some(),
        "silence row must exist in the keyboard"
    );
    let data = silence_row.and_then(|b| b.callback_data.as_deref());
    assert_eq!(data, Some("cb:al:abc:silent"));
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

#[test]
fn test_is_already_stripped_to_checks_file_tag() {
    let tmp = std::env::temp_dir().join(format!("ryoiki_tag_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let hindi = tmp.join("The Bads - S01E01 [Hindi] [1080p].mkv");
    std::fs::write(&hindi, b"test").unwrap_or_default();

    assert!(is_already_stripped_to(&hindi, "Hindi"));
    assert!(is_already_stripped_to(&hindi, "hindi"));
    assert!(!is_already_stripped_to(&hindi, "Tamil"));

    let non_existent = tmp.join("NonExistent [Hindi].mkv");
    assert!(!is_already_stripped_to(&non_existent, "Hindi"));
}

#[test]
fn test_locate_media_path_finds_existing() {
    let tmp = std::env::temp_dir().join(format!("ryoiki_loc_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    let sample = tmp.join("sample.mkv");
    std::fs::write(&sample, b"test").unwrap_or_default();

    let found = decisions::locate_media_path(&sample);
    assert_eq!(found, Some(sample));
}
