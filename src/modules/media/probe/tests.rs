use super::*;

#[test]
fn test_parse_ffprobe_json() {
    let sample = br#"{
        "streams": [
            {
                "codec_type": "video",
                "width": 1920,
                "height": 800,
                "tags": { "language": "mal" }
            },
            {
                "codec_type": "audio",
                "tags": { "language": "mal" }
            }
        ],
        "format": {
            "duration": "9854.272000"
        }
    }"#;

    let Some(probe) = parse_ffprobe_json(sample) else {
        panic!("Failed to parse sample json");
    };
    assert_eq!(probe.duration_mins, Some(164));
    assert_eq!(probe.resolution.as_deref(), Some("1080p"));
    assert_eq!(probe.audio_languages, vec!["Malayalam"]);
    assert_eq!(probe.primary_language.as_deref(), Some("Malayalam"));
}

#[test]
fn test_resolve_primary_language() {
    assert_eq!(
        resolve_primary_language(&["Malayalam".to_string()]).as_deref(),
        Some("Malayalam")
    );
    assert_eq!(
        resolve_primary_language(&["Malayalam".to_string(), "English".to_string()]).as_deref(),
        Some("Malayalam")
    );
    assert_eq!(
        resolve_primary_language(&["Malayalam".to_string(), "Tamil".to_string()]).as_deref(),
        Some("Multi")
    );
    assert_eq!(
        resolve_primary_language(&["English".to_string()]).as_deref(),
        Some("English")
    );
}

#[test]
fn test_canonical_resolution_for_height() {
    assert_eq!(
        canonical_resolution_for_height(2160).as_deref(),
        Some("2160p")
    );
    assert_eq!(
        canonical_resolution_for_height(1080).as_deref(),
        Some("1080p")
    );
    assert_eq!(
        canonical_resolution_for_height(720).as_deref(),
        Some("720p")
    );
    assert_eq!(
        canonical_resolution_for_height(480).as_deref(),
        Some("480p")
    );
    assert_eq!(canonical_resolution_for_height(360), None);
}

#[test]
fn test_resolution_for_dimensions_keeps_cropped_1080p() {
    // A 2.40:1 cinematic master cropped to 1920x800 is still 1080p-class.
    assert_eq!(
        resolution_for_dimensions(1920, 800).as_deref(),
        Some("1080p")
    );
    assert_eq!(
        resolution_for_dimensions(1920, 804).as_deref(),
        Some("1080p")
    );
    assert_eq!(
        resolution_for_dimensions(1280, 720).as_deref(),
        Some("720p")
    );
    assert_eq!(
        resolution_for_dimensions(3840, 2160).as_deref(),
        Some("2160p")
    );
    assert_eq!(resolution_for_dimensions(720, 480).as_deref(), Some("480p"));
}

#[test]
fn test_normalize_resolution_collapses_variants() {
    assert_eq!(normalize_resolution("1024x768"), "720p");
    assert_eq!(normalize_resolution("1024×768"), "720p");
    assert_eq!(normalize_resolution("768p"), "720p");
    assert_eq!(normalize_resolution("1920x1080"), "1080p");
    assert_eq!(normalize_resolution("1920x800"), "1080p");
    assert_eq!(normalize_resolution("1080p"), "1080p");
    assert_eq!(normalize_resolution("1280x720"), "720p");
    assert_eq!(normalize_resolution("480p"), "480p");
    assert_eq!(normalize_resolution("2160p"), "2160p");
    assert_eq!(normalize_resolution("360p"), "360p");
}

#[test]
fn test_normalize_resolution_ignores_non_resolutions() {
    assert_eq!(normalize_resolution(""), "");
    assert_eq!(normalize_resolution("x264 AAC"), "x264 AAC");
    assert_eq!(normalize_resolution("Japanese"), "Japanese");
}

#[test]
fn test_is_resolution_label() {
    for good in ["768p", "1080p", "1024x768", "1920x1080", "480p"] {
        assert!(is_resolution_label(good), "{good} should be a label");
    }
    for bad in ["x264 AAC", "x265 HEVC", "Japanese", "SP01", ""] {
        assert!(!is_resolution_label(bad), "{bad} should not be a label");
    }
}

/// End-to-end probe matrix over real generated video files.
///
/// Set `RYOIKI_PROBE_MATRIX` to a directory of `<Name> (2020).mkv` files whose
/// names carry no resolution hint, so only ffprobe can set the tag. Skipped
/// (returns early) when the env var is unset, keeping CI hermetic.
#[test]
fn test_probe_matrix_from_dir() {
    let Ok(dir) = std::env::var("RYOIKI_PROBE_MATRIX") else {
        return;
    };
    let cases: &[(&str, Option<&str>)] = &[
        ("Std2160", Some("2160p")),
        ("Std1440", Some("1080p")),
        ("Std1080", Some("1080p")),
        ("Std768", Some("720p")),
        ("Std720", Some("720p")),
        ("Std480", Some("480p")),
        ("Std360", None),
        ("Crop240_1080", Some("1080p")),
        ("Crop239_1080", Some("1080p")),
        ("Crop235_1080", Some("1080p")),
        ("Crop240_720", Some("720p")),
        ("Crop235_720", Some("720p")),
        ("Crop240_2160", Some("2160p")),
        ("Crop235_2160", Some("2160p")),
        ("Anamorph4_3_HD", Some("1080p")),
        ("Anamorph4_3_720", Some("720p")),
        ("PAL_DVD", Some("480p")),
        ("NTSC_DVD", Some("480p")),
        ("Ultrawide2160", Some("2160p")),
        ("Ultrawide1440", Some("1080p")),
        ("Ultrawide1080", Some("1080p")),
        ("Vertical1080", Some("1080p")),
        ("Vertical720", Some("720p")),
        ("RawH_1000", Some("1080p")),
        ("RawH_998", Some("720p")),
        ("RawH_700", Some("720p")),
        ("RawH_698", Some("480p")),
        ("RawH_450", Some("480p")),
        ("RawH_448", None),
        ("RawH_360", None),
    ];

    let mut failures = Vec::new();
    for (name, expected) in cases {
        let path = std::path::Path::new(&dir).join(format!("{name} (2020).mkv"));
        let probe = probe_media_file(&path);
        let Some(probe) = probe else {
            failures.push(format!("{name}: ffprobe returned None"));
            continue;
        };
        let got = probe.resolution.as_deref();
        let status = if got == *expected { "ok" } else { "FAIL" };
        println!("{name:<16} expected={expected:?} got={got:?}  {status}");
        if got != *expected {
            failures.push(format!("{name}: expected {expected:?}, got {got:?}"));
        }
    }

    assert!(failures.is_empty(), "probe matrix failures:\n{}", failures.join("\n"));
}
