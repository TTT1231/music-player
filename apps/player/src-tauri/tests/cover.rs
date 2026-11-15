use std::fs;
use std::path::PathBuf;

use music_player_shell_lib::cover::{CoverCache, cover_response, decode_cover_id, encode_cover_id};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/music")
        .join(name)
}

#[test]
fn encoded_identifiers_round_trip_and_reject_invalid_utf8() {
    let id = "歌手 / #?% - song";
    assert_eq!(decode_cover_id(&encode_cover_id(id)).as_deref(), Some(id));
    assert_eq!(decode_cover_id("%FF"), None);
}

#[test]
fn embedded_cover_is_cached_and_changing_the_path_invalidates_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("music.mp3");
    fs::copy(fixture("Test Artist、Guest - Track One (Live).mp3"), &path).unwrap();
    let mut covers = CoverCache::default();
    let first = covers.get("track", &path).expect("fixture has a cover");
    assert_eq!(first.mime, "image/png");
    fs::remove_file(&path).unwrap();
    assert_eq!(covers.get("track", &path), Some(first));
    assert_eq!(covers.get("track", &fixture("No Artist Instrumental.mp3")), None);
}

#[test]
fn protocol_responses_allow_canvas_access_and_disable_stale_browser_cache() {
    let mut covers = CoverCache::default();
    let image = covers.get("track", &fixture("Test Artist、Guest - Track One (Live).mp3"));
    let response = cover_response(image);
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["access-control-allow-origin"], "*");
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["content-type"], "image/png");
    let absent = cover_response(None);
    assert_eq!(absent.status(), 404);
    assert_eq!(absent.headers()["access-control-allow-origin"], "*");
}
