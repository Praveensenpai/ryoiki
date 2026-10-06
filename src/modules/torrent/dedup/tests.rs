//! Tests for magnet parsing, dedup availability, and Drive restore.

use super::*;
use std::sync::Mutex;

/// Serializes tests that mutate process-global env vars (Cargo runs tests in
/// parallel threads, so unlocked `set_var` calls would race each other).
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_parse_magnet_raw_hash() {
    let hash = "0123456789abcdef0123456789abcdef01234567";
    let (parsed_hash, dn) = parse_magnet(hash);
    assert_eq!(parsed_hash.as_deref(), Some(hash));
    assert!(dn.is_none());
}

#[test]
fn test_parse_magnet_with_dn() {
    let mag = "magnet:?xt=urn:btih:3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c&dn=Sousou%20no%20Frieren%20-%2001%20%5B1080p%5D.mkv";
    let (parsed_hash, dn) = parse_magnet(mag);
    assert_eq!(
        parsed_hash.as_deref(),
        Some("3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c")
    );
    assert_eq!(dn.as_deref(), Some("Sousou no Frieren - 01 [1080p].mkv"));
}

#[test]
fn test_url_decode() {
    assert_eq!(url_decode("Hello%20World%2BTest"), "Hello World+Test");
    assert_eq!(url_decode("Title%20%5B1080p%5D"), "Title [1080p]");
}

#[test]
fn test_extract_rel_media_subpath() {
    let path = PathBuf::from("/home/neko/jellyfin/media/anime/Show/Season 01/ep1.mkv");
    let rel = extract_rel_media_subpath(&path);
    assert_eq!(rel, Some(PathBuf::from("anime/Show/Season 01/ep1.mkv")));
}

#[test]
fn test_check_already_available_untracked_is_not_available() {
    let mag = "magnet:?xt=urn:btih:ffffffffffffffffffffffffffffffffffffffff&dn=RandomMovie.mkv";
    let res = check_already_available(mag);
    assert!(matches!(res, Availability::NotAvailable { .. }));
}

#[test]
fn test_restore_from_cloud_dual_versions() -> Result<()> {
    let tmp = std::env::temp_dir().join(format!("ryoiki_test_restore_{}", std::process::id()));
    let cloud_dir = tmp.join("cloud");
    let local_dir = tmp.join("local");
    let _ = fs::create_dir_all(&cloud_dir);
    let _ = fs::create_dir_all(&local_dir);

    let src_orig = cloud_dir.join("Show - S01E01 [1080p].mkv");
    let src_multi = cloud_dir.join("Show - S01E01 [1080p] [Multi].mkv");
    fs::write(&src_orig, "orig_content")?;
    fs::write(&src_multi, "multi_content")?;

    let dest_orig = local_dir.join("Show - S01E01 [1080p].mkv");
    let dest_multi = local_dir.join("Show - S01E01 [1080p] [Multi].mkv");

    let pairs = vec![
        (src_orig.clone(), dest_orig.clone()),
        (src_multi.clone(), dest_multi.clone()),
    ];
    let restored = restore_from_cloud(&pairs)?;
    assert_eq!(restored.len(), 2);
    assert!(dest_orig.exists());
    assert!(dest_multi.exists());
    assert_eq!(fs::read_to_string(&dest_orig)?, "orig_content");
    assert_eq!(fs::read_to_string(&dest_multi)?, "multi_content");

    let _ = fs::remove_dir_all(&tmp);
    Ok(())
}

#[test]
fn test_dedup_full_lifecycle_three_examples() -> Result<()> {
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let tmp = std::env::temp_dir().join(format!("ryoiki_lifecycle_{}", std::process::id()));
    // Sandbox persisted history so this test never writes to the real profile.
    std::env::set_var("RYOIKI_DATA_DIR", tmp.join("data"));

    let diff_magnet =
        "magnet:?xt=urn:btih:9999999999999999999999999999999999999999&dn=Frieren+1080p+HighBitrate.mkv";
    let res = check_already_available(diff_magnet);
    assert!(matches!(res, Availability::NotAvailable { .. }));

    let local_dir = tmp.join("jellyfin/media/anime/Frieren/Season 01");
    let _ = fs::create_dir_all(&local_dir);
    let orig_file = local_dir.join("Frieren - S01E01 [1080p].mkv");
    let multi_file = local_dir.join("Frieren - S01E01 [1080p] [Multi].mkv");
    fs::write(&orig_file, "orig_video")?;
    fs::write(&multi_file, "multi_video")?;

    let tracked = vec![
        crate::modules::torrent::history::create_tracked_file(orig_file.clone(), "original"),
        crate::modules::torrent::history::create_tracked_file(multi_file.clone(), "multi"),
    ];
    let info =
        crate::modules::media::heuristic::classify_media_heuristic("Frieren - S01E01 [1080p].mkv");
    let known_hash = "8888888888888888888888888888888888888888";
    crate::modules::torrent::history::record_download_history(Some(known_hash), &info, tracked)?;

    let known_magnet = format!("magnet:?xt=urn:btih:{known_hash}&dn=Frieren+S01E01.mkv");
    let res_local = check_already_available(&known_magnet);
    match res_local {
        Availability::Local { paths, title } => {
            assert_eq!(title, "Frieren");
            assert_eq!(paths.len(), 2);
            assert!(paths.contains(&orig_file));
            assert!(paths.contains(&multi_file));
        }
        _ => panic!("Expected Availability::Local, got {res_local:?}"),
    }

    // --- Partial availability: only the original is local; the multi variant
    // lives in Drive. check_already_available must restore it.
    let _ = fs::remove_file(&multi_file);
    assert!(orig_file.exists() && !multi_file.exists());

    let gdrive_base = tmp.join("gdrive");
    std::env::set_var("RYOIKI_GDRIVE_DIR", &gdrive_base);
    let gdrive_dir = gdrive_base.join("media/anime/Frieren/Season 01");
    let _ = fs::create_dir_all(&gdrive_dir);
    let gdrive_multi = gdrive_dir.join("Frieren - S01E01 [1080p] [Multi].mkv");
    fs::write(&gdrive_multi, "cloud_multi_video")?;

    let res_partial = check_already_available(&known_magnet);
    match res_partial {
        Availability::Local { paths, .. } => {
            assert!(paths.contains(&orig_file), "local variant kept");
            assert!(
                multi_file.exists(),
                "missing variant must be restored from Drive"
            );
            assert_eq!(fs::read_to_string(&multi_file)?, "cloud_multi_video");
        }
        _ => panic!("Expected Local with restored multi, got {res_partial:?}"),
    }

    std::env::remove_var("RYOIKI_GDRIVE_DIR");
    std::env::remove_var("RYOIKI_DATA_DIR");
    let _ = fs::remove_dir_all(&tmp);
    Ok(())
}

#[test]
fn test_find_in_google_drive_remaps_movies_category() -> Result<()> {
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let tmp = std::env::temp_dir().join(format!("ryoiki_gdrive_remap_{}", std::process::id()));
    let base = tmp.join("gdrive");
    std::env::set_var("RYOIKI_GDRIVE_DIR", &base);

    let local_path = PathBuf::from("/home/neko/jellyfin/media/movies/Some Film (2024)/film.mkv");
    let archived = base.join("media/movie/Some Film (2024)/film.mkv");
    fs::create_dir_all(archived.parent().unwrap_or(&base))?;
    fs::write(&archived, "archived")?;

    let found = find_in_google_drive(&local_path);
    assert_eq!(found.as_deref(), Some(archived.as_path()));

    std::env::remove_var("RYOIKI_GDRIVE_DIR");
    let _ = fs::remove_dir_all(&tmp);
    Ok(())
}
