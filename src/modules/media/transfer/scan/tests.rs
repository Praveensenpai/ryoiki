use super::*;

#[test]
fn test_remote_subpath_maps_movies_singular() {
    assert_eq!(remote_subpath("movies"), "movie");
    assert_eq!(remote_subpath("movies/Title (2024)"), "movie/Title (2024)");
}

#[test]
fn test_remote_subpath_preserves_other_categories() {
    assert_eq!(remote_subpath("shows"), "shows");
    assert_eq!(remote_subpath("anime"), "anime");
    assert_eq!(remote_subpath("anime/movie"), "anime/movie");
}

#[test]
fn test_scan_local_category_flat_anime() -> Result<()> {
    let temp_dir = std::env::temp_dir().join("test_scan_flat_anime");
    let anime_dir = temp_dir.join("anime").join("Kimetsu no Yaiba (2025)");
    fs::create_dir_all(&anime_dir)?;
    let file_path = anime_dir.join("Kimetsu no Yaiba (2025) [1080p].mkv");
    fs::write(&file_path, b"1234567890")?;

    let mut items = Vec::new();
    let mut found_keys = HashSet::new();
    let mut cache = MediaScanCache::default();

    scan_local_category(
        &temp_dir,
        "anime",
        MediaCategory::Anime,
        &mut items,
        &mut found_keys,
        &mut cache,
    );

    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item.title, "Kimetsu no Yaiba (2025)");
    assert_eq!(item.category, MediaCategory::Anime);
    assert_eq!(item.size_bytes, 10);
    assert!(item.seasons.is_empty());
    assert_eq!(item.files.len(), 1);
    assert_eq!(item.files[0].name, "Kimetsu no Yaiba (2025) [1080p].mkv");

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn test_scan_local_anime_movies_subfolder() -> Result<()> {
    let temp_dir = std::env::temp_dir().join(format!("test_anime_movies_{}", std::process::id()));
    let anime_movie_dir = temp_dir.join("anime/movie/Koe no Katachi (2016)");
    fs::create_dir_all(&anime_movie_dir)?;
    let file_path = anime_movie_dir.join("Koe no Katachi (2016) [Japanese] [1080p].mkv");
    fs::write(&file_path, b"koe_no_katachi_bytes")?;

    let mut items = Vec::new();
    let mut found_keys = HashSet::new();
    let mut cache = MediaScanCache::default();

    // Scanning "anime" must skip "movie"
    scan_local_category(
        &temp_dir,
        "anime",
        MediaCategory::Anime,
        &mut items,
        &mut found_keys,
        &mut cache,
    );
    assert!(
        items.is_empty(),
        "anime/movie should not be added as an item titled 'movie'"
    );

    // Scanning "anime/movie" must discover Koe no Katachi (2016) directly
    scan_local_category(
        &temp_dir,
        "anime/movie",
        MediaCategory::Anime,
        &mut items,
        &mut found_keys,
        &mut cache,
    );
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Koe no Katachi (2016)");
    assert_eq!(items[0].category, MediaCategory::Anime);
    assert!(items[0].seasons.is_empty());
    assert_eq!(items[0].files.len(), 1);

    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}
