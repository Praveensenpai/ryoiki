use super::*;
use crate::modules::media::ClassificationEngine;

#[test]
fn test_group_videos_by_series_splits_sibling_series() {
    let target = PathBuf::from("/t");
    let files = vec![
        PathBuf::from("/t/Laid-Back Camp/Season 1/ep1.mkv"),
        PathBuf::from("/t/Laid-Back Camp/Season 1/ep2.mkv"),
        PathBuf::from("/t/Room Camp/Season 1/ep1.mkv"),
    ];
    let groups = group_videos_by_series(&target, files);
    assert_eq!(groups.len(), 2, "sibling series must not share a batch");
    assert_eq!(groups.values().map(Vec::len).sum::<usize>(), 3);
    assert!(groups.values().any(|g| g.len() == 2));
    assert!(groups.values().any(|g| g.len() == 1));
}

#[test]
fn test_group_videos_by_series_flat_stays_together() {
    let target = PathBuf::from("/t");
    let files = vec![
        PathBuf::from("/t/Show.S01E01.mkv"),
        PathBuf::from("/t/Show.S01E02.mkv"),
    ];
    let groups = group_videos_by_series(&target, files);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups.values().map(Vec::len).sum::<usize>(), 2);
}

#[test]
fn test_group_videos_by_series_structural_subfolder_stays_with_parent() {
    let target = PathBuf::from("/t");
    let files = vec![
        PathBuf::from("/t/Onegai My Melody Shuffle/Episode 01.mkv"),
        PathBuf::from("/t/Onegai My Melody Shuffle/EXTRA/Kurukuru Shuffle [SP01] NCOP.mkv"),
    ];
    let groups = group_videos_by_series(&target, files);
    assert_eq!(groups.len(), 1, "EXTRA must not spawn a phantom series");
    assert_eq!(groups.values().map(Vec::len).sum::<usize>(), 2);
}

#[test]
fn test_is_structural_dir_recognizes_known_folders() {
    for name in ["EXTRA", "Specials", "Season 1", "SP", "NCED", "CM", "Bonus"] {
        assert!(is_structural_dir(name), "{name} should be structural");
    }
    for name in ["Room Camp", "Heya Camp", "Laid-Back Camp"] {
        assert!(!is_structural_dir(name), "{name} should not be structural");
    }
}

#[test]
fn test_adjust_single_audio_replaces_multi_with_probed_lang() {
    let mut info = super::super::MediaInfo {
        media_type: MediaType::Show,
        title: "Anime Show".to_string(),
        year: Some(2024),
        season: Some(1),
        episode: Some(1),
        resolution: Some("1080p".to_string()),
        language: Some("Multi".to_string()),
        clean_name: "Anime Show - S01E01 [Multi] [1080p].mkv".to_string(),
        is_extra: false,
        engine: ClassificationEngine::Ai,
    };

    let probe = MediaProbe {
        duration_mins: Some(24),
        resolution: Some("1080p".to_string()),
        audio_stream_count: 1,
        audio_languages: vec!["Japanese".to_string()],
        primary_language: Some("Japanese".to_string()),
    };

    adjust_media_info_post_classify(&mut info, Some(&probe));
    assert_eq!(info.language.as_deref(), Some("Japanese"));
    assert_eq!(
        info.clean_name,
        "Anime Show - S01E01 [Japanese] [1080p].mkv"
    );
    assert_eq!(info.media_type, MediaType::Anime);
}

#[test]
fn test_adjust_single_audio_unknown_lang_replaces_multi_with_original() {
    let mut info = super::super::MediaInfo {
        media_type: MediaType::Movie,
        title: "Indie Film".to_string(),
        year: Some(2023),
        season: None,
        episode: None,
        resolution: Some("1080p".to_string()),
        language: Some("Multi".to_string()),
        clean_name: "Indie Film (2023) [Multi] [1080p].mkv".to_string(),
        is_extra: false,
        engine: ClassificationEngine::Ai,
    };

    let probe = MediaProbe {
        duration_mins: Some(90),
        resolution: Some("1080p".to_string()),
        audio_stream_count: 1,
        audio_languages: vec![],
        primary_language: None,
    };

    adjust_media_info_post_classify(&mut info, Some(&probe));
    assert_eq!(info.language.as_deref(), Some("Original"));
    assert_eq!(info.clean_name, "Indie Film (2023) [Original] [1080p].mkv");
}

#[test]
fn test_adjust_multi_audio_ensures_multi_tag() {
    let mut info = super::super::MediaInfo {
        media_type: MediaType::Movie,
        title: "Blockbuster".to_string(),
        year: Some(2024),
        season: None,
        episode: None,
        resolution: Some("1080p".to_string()),
        language: Some("English".to_string()),
        clean_name: "Blockbuster (2024) [English] [1080p].mkv".to_string(),
        is_extra: false,
        engine: ClassificationEngine::Ai,
    };

    let probe = MediaProbe {
        duration_mins: Some(120),
        resolution: Some("1080p".to_string()),
        audio_stream_count: 3,
        audio_languages: vec![
            "English".to_string(),
            "Hindi".to_string(),
            "Tamil".to_string(),
        ],
        primary_language: Some("Multi".to_string()),
    };

    adjust_media_info_post_classify(&mut info, Some(&probe));
    assert_eq!(info.language.as_deref(), Some("Multi"));
    assert_eq!(info.clean_name, "Blockbuster (2024) [Multi] [1080p].mkv");
}
