use super::*;
use anyhow::{Context, Result};
use std::fs;

#[test]
fn test_find_dubstrip_bin() {
    let _ = find_dubstrip_bin();
}

#[test]
fn test_finalize_dual_versions_creates_both_files() -> Result<()> {
    let tmp = std::env::temp_dir().join(format!("ryoiki_test_dual_{}", std::process::id()));
    fs::create_dir_all(&tmp)?;

    let path = tmp.join("Sample Movie (2024) [Multi] [1080p].mkv");
    let temp_multi = tmp.join(".Sample Movie (2024) [Multi] [1080p].mkv.multi_tmp");

    fs::write(&path, b"mock stripped native audio")?;
    fs::write(&temp_multi, b"mock multi audio")?;

    let res = finalize_dual_versions(&path, &temp_multi, "Japanese");
    let (orig_p, multi_p) = res.context("Expected dual versions to be created")?;

    assert_eq!(
        orig_p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default(),
        "Sample Movie (2024) [Japanese] [1080p].mkv"
    );
    assert_eq!(
        multi_p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default(),
        "Sample Movie (2024) [Multi] [1080p].mkv"
    );

    assert!(orig_p.exists());
    assert!(multi_p.exists());
    assert_eq!(fs::read_to_string(&orig_p)?, "mock stripped native audio");
    assert_eq!(fs::read_to_string(&multi_p)?, "mock multi audio");

    let _ = fs::remove_dir_all(&tmp);
    Ok(())
}

#[test]
fn test_finalize_single_version_has_no_multi() -> Result<()> {
    let tmp = std::env::temp_dir().join(format!("ryoiki_test_single_{}", std::process::id()));
    fs::create_dir_all(&tmp)?;

    let path = tmp.join("Anime Show (2024) [Multi] [1080p].mkv");
    fs::write(&path, b"mock japanese only")?;

    let orig = finalize_single_version(&path, "Japanese")
        .context("Expected single version to be created")?;
    assert_eq!(
        orig.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default(),
        "Anime Show (2024) [Japanese] [1080p].mkv"
    );
    assert!(orig.exists());
    assert!(!tmp.join("Anime Show (2024) [Multi] [1080p].mkv").exists());

    let _ = fs::remove_dir_all(&tmp);
    Ok(())
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
    a.record(&StripOutcome::Preserved);
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
