use std::fs;
use std::path::PathBuf;

use music_player_shell_lib::controller::path_resolver;
use music_player_shell_lib::cover::{CoverCache, cover_response, decode_cover_id, encode_cover_id, sidecar_image};
use music_player_shell_lib::ipc::{load_cover, load_cover_for_track};
use music_player_shell_lib::library_files::LibraryFiles;

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

/// 管理布局：`<曲库>/assets/music/<曲>` 旁边放 `<曲库>/assets/img/<同名>`。
fn library_layout(dir: &std::path::Path, audio_stem: &str, image_stem: Option<&str>) -> PathBuf {
    let music = dir.join("assets/music");
    let img = dir.join("assets/img");
    fs::create_dir_all(&music).unwrap();
    fs::create_dir_all(&img).unwrap();
    fs::copy(
        fixture("No Artist Instrumental.mp3"),
        music.join(format!("{audio_stem}.mp3")),
    )
    .unwrap();
    if let Some(stem) = image_stem {
        fs::copy(
            fixture("img/No Artist Instrumental.jpg"),
            img.join(format!("{stem}.jpg")),
        )
        .unwrap();
    }
    music.join(format!("{audio_stem}.mp3"))
}

#[test]
fn sidecar_image_matches_the_audio_name_regardless_of_case_or_padding() {
    let dir = tempfile::tempdir().unwrap();
    // 音频文件名带尾随空格、图片扩展名大小写不同：按 trim + 忽略大小写匹配
    let audio = library_layout(dir.path(), "song ", Some("SONG"));
    let cover = sidecar_image(&audio).expect("同名图片应作为封面回退");
    assert_eq!(cover.mime, "image/jpeg");
    assert!(cover.data.starts_with(&[0xFF, 0xD8, 0xFF]), "应为 JPEG 字节");

    // 只有非图片扩展名的同名文件时不误报
    fs::remove_file(dir.path().join("assets/img/SONG.jpg")).unwrap();
    fs::write(dir.path().join("assets/img/song.gif"), b"GIF89a").unwrap();
    assert_eq!(sidecar_image(&audio), None);
}

#[test]
fn cover_lookup_prefers_embedded_art_and_falls_back_to_the_sidecar() {
    let resolver = path_resolver();
    let dir = tempfile::tempdir().unwrap();

    // 无内嵌封面：回退到曲库 img/ 里的同名图片
    let without = library_layout(dir.path(), "No Artist Instrumental", Some("No Artist Instrumental"));
    let fallback = load_cover(&resolver, &without).expect("无内嵌封面时应回退同名图片");
    assert_eq!(fallback.mime, "image/jpeg");

    // 内嵌封面优先于同名图片
    let stem = "Test Artist、Guest - Track One (Live)";
    let audio = dir.path().join("assets/music").join(format!("{stem}.mp3"));
    fs::copy(fixture(&format!("{stem}.mp3")), &audio).unwrap();
    fs::copy(
        fixture("img/No Artist Instrumental.jpg"),
        dir.path().join("assets/img").join(format!("{stem}.jpg")),
    )
    .unwrap();
    let embedded = load_cover(&resolver, &audio).unwrap();
    assert_eq!(embedded.mime, "image/png");

    // 两者皆无：维持无封面
    let bare = library_layout(dir.path(), "孤儿", None);
    assert_eq!(load_cover(&resolver, &bare), None);
}

#[test]
fn library_cover_is_found_for_a_legacy_android_content_uri() {
    use std::sync::Arc;

    let dir = tempfile::tempdir().unwrap();
    let image_dir = dir.path().join("assets/img");
    fs::create_dir_all(&image_dir).unwrap();
    fs::copy(fixture("img/No Artist Instrumental.jpg"), image_dir.join(" SONG .JPG")).unwrap();
    let audio = fixture("No Artist Instrumental.mp3");
    let resolver: music_player_shell_lib::controller::LocationResolver =
        Arc::new(move |_: &str| Ok((fs::File::open(&audio)?, Some("mp3".to_owned()))));
    let library = LibraryFiles::new(dir.path().join("assets"));

    let cover = load_cover_for_track(&resolver, std::path::Path::new("content://media/42"), &library, "song")
        .expect("旧安卓 URI 的歌曲也应能读取曲库中的同名图片");
    assert_eq!(cover.mime, "image/jpeg");
}
