use super::*;

#[test]
fn test_find_dubstrip_bin() {
    let _ = find_dubstrip_bin();
}

#[test]
fn test_audio_summary_records_and_merges() {
    let mut a = AudioStripSummary::default();
    a.record(&StripOutcome::Stripped {
        primary_lang: "Japanese".to_string(),
        original_path: PathBuf::from("/a.mkv"),
        multi_path: None,
        reclaimed_bytes: 1024 * 1024,
    });
    a.record(&StripOutcome::Preserved {
        path: PathBuf::from("/a.mkv"),
    });
    a.record(&StripOutcome::Failed);

    assert_eq!(a.stripped, 1);
    assert_eq!(a.preserved, 1);
    assert_eq!(a.failed, 1);
    assert_eq!(a.reclaimed_bytes, 1024 * 1024);
    assert!(a.render_html_line().is_some());

    let mut b = AudioStripSummary::default();
    b.merge(&a);
    assert_eq!(b.stripped, 1);
    assert_eq!(b.reclaimed_bytes, 1024 * 1024);
}

#[test]
fn test_empty_summary_renders_nothing() {
    let s = AudioStripSummary::default();
    assert!(s.is_empty());
    assert!(s.render_html_line().is_none());
}

#[test]
fn test_format_reclaimed_units() {
    assert_eq!(format_reclaimed(512), "512 B");
    assert_eq!(format_reclaimed(2048), "2.0 KB");
}
