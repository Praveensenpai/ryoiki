use super::*;

#[test]
fn test_fallback_action_message_duplicate_reports_downloading() {
    assert_eq!(
        fallback_action_message(api::QbEnsure::AlreadyPresent),
        "Already downloading in qBittorrent (queued at submit)"
    );
    assert_eq!(
        fallback_action_message(api::QbEnsure::Failed),
        "Failed to forward to qBittorrent"
    );
}

#[test]
fn test_extract_btih_hash() {
    let magnet = "magnet:?xt=urn:btih:8A47A4DAC599B86BB196DE26F7EFFA2D7C0B9E1C&dn=Test";
    let hash = extract_btih_hash(magnet);
    assert_eq!(
        hash.as_deref(),
        Some("8a47a4dac599b86bb196de26f7effa2d7c0b9e1c")
    );
}

#[test]
fn test_cleanup_qbittorrent_removes_matching_torrent() {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| Client::new());
    let qb_url = "http://localhost:6881";
    if api::get_torrents(&client, qb_url, None).is_ok() {
        cleanup_qbittorrent(
            &client,
            qb_url,
            Some("3333333333333333333333333333333333333333"),
            "Karakuri Test Drive S01E01.mkv",
        );
        cleanup_qbittorrent(&client, qb_url, None, "Akkun to Kanojo S01E01.mkv");
        let after = api::get_torrents(&client, qb_url, None).unwrap_or_default();
        assert!(!after
            .iter()
            .any(|t| t.hash == "3333333333333333333333333333333333333333"));
        assert!(!after
            .iter()
            .any(|t| t.hash == "1111111111111111111111111111111111111111"));
    }
}
