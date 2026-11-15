//! 验收：导入与播放列表。
//! 歌曲和 `.lrc` 按文件名自动配对；只选 `.lrc` 时补给同名歌曲；同名歌曲原位替换；
//! 导入文件夹包含子文件夹；移出列表不删除文件。

mod common;

use common::{file, fixtures, playlist, titles};
use music_core::{FileRef, Playlist, is_audio_file, scan_dir};

#[test]
fn recognises_supported_audio_extensions_case_insensitively() {
    for name in ["a.mp3", "a.MP3", "a.ogg", "a.flac", "a.m4a", "a.wav"] {
        assert!(is_audio_file(name), "{name}");
    }
    for name in ["a.lrc", "a.jpg", "a.txt", "mp3", "a"] {
        assert!(!is_audio_file(name), "{name}");
    }
}

#[test]
fn pairs_song_and_lyrics_by_name_ignoring_case_and_surrounding_spaces() {
    let mut list = Playlist::new();
    let report = list.import([
        file("歌手 - Song.MP3"),
        file(" 歌手 - song .Lrc"),
        file("歌手 - 另一首.ogg"),
    ]);

    assert_eq!(report.added.len(), 2);
    assert_eq!(report.with_lyrics, 1);
    let song = &list.tracks()[0];
    assert_eq!((song.artist.as_str(), song.title.as_str()), ("歌手", "Song"));
    assert_eq!(song.lyrics.as_deref(), Some("/music/ 歌手 - song .Lrc"));
    assert_eq!(list.tracks()[1].lyrics, None);
}

#[test]
fn keeps_import_order_and_ignores_other_files() {
    let mut list = Playlist::new();
    let report = list.import([
        file("b - 二.mp3"),
        file("cover.jpg"),
        file("a - 一.ogg"),
        file("说明.txt"),
    ]);
    assert_eq!(titles(&list), ["二", "一"]);
    assert_eq!(report.added, common::ids(&list));
}

#[test]
fn lyrics_only_attach_to_existing_song_and_orphans_are_dropped() {
    let mut list = playlist(&["歌手 - 有歌", "歌手 - 另一首"]);
    let report = list.import([file("歌手 - 有歌.lrc"), file("歌手 - 不存在.lrc")]);

    assert_eq!(report.lyrics_attached, 1);
    assert_eq!(report.orphan_lyrics, 1);
    assert!(report.added.is_empty());
    assert_eq!(list.len(), 2);
    assert_eq!(list.tracks()[0].lyrics.as_deref(), Some("/music/歌手 - 有歌.lrc"));
}

#[test]
fn reimporting_same_name_replaces_in_place_without_duplicates() {
    let mut list = playlist(&["甲 - 一", "乙 - 二", "丙 - 三"]);
    let before = common::ids(&list);
    let report = list.import([FileRef::new("/新位置/乙 - 二.ogg", "乙 - 二.ogg"), file("乙 - 二.lrc")]);

    assert_eq!(common::ids(&list), before);
    assert_eq!(report.added, [before[1].clone()]);
    let replaced = &list.tracks()[1];
    assert_eq!(replaced.audio, "/新位置/乙 - 二.ogg");
    assert!(replaced.lyrics.is_some());
}

#[test]
fn folder_import_includes_subfolders() {
    let files = scan_dir(&fixtures()).unwrap();
    let mut list = Playlist::new();
    let report = list.import(files);

    assert_eq!(report.added.len(), 4);
    assert_eq!(report.with_lyrics, 3);
    let third = list
        .tracks()
        .iter()
        .find(|t| t.title == "Track Three")
        .expect("子文件夹里的歌");
    assert!(third.audio.contains("subfolder"));
    assert!(third.lyrics.as_deref().is_some_and(|l| l.ends_with(".lrc")));
}

#[test]
fn file_ref_from_path_uses_path_as_location() {
    let path = fixtures().join("No Artist Instrumental.mp3");
    let f = FileRef::from_path(&path);
    assert_eq!(f.file_name, "No Artist Instrumental.mp3");
    assert_eq!(f.location, path.to_string_lossy());
}

#[test]
fn removing_from_list_keeps_the_file_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("歌手 - 留着.mp3");
    std::fs::copy(fixtures().join("No Artist Instrumental.mp3"), &path).unwrap();

    let mut list = Playlist::new();
    list.import([FileRef::from_path(&path)]);
    let id = list.tracks()[0].id.clone();
    assert!(list.remove(&id).is_some());

    assert!(list.is_empty());
    assert!(path.exists());
}

#[test]
fn move_track_changes_order() {
    let mut list = playlist(&["a - 一", "a - 二", "a - 三", "a - 四"]);
    list.move_track(0, 2);
    assert_eq!(titles(&list), ["二", "三", "一", "四"]);
    list.move_track(3, 0);
    assert_eq!(titles(&list), ["四", "二", "三", "一"]);
}
