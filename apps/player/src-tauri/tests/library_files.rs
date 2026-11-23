use std::{
    fs, io,
    path::Path,
    sync::{Arc, atomic::AtomicU64},
};

use music_core::{FileRef, load_session};
use music_player_shell_lib::{
    audio_backend::DeviceBackend,
    controller::{Controller, PlatformBehavior, path_resolver},
    library_files::LibraryFiles,
};

fn source(dir: &Path, name: &str, data: &[u8]) -> FileRef {
    let path = dir.join(name);
    fs::write(&path, data).unwrap();
    FileRef::from_path(&path)
}

fn controller(session: &Path) -> Controller<DeviceBackend> {
    Controller::new(
        session.to_owned(),
        DeviceBackend::unavailable("test".into()),
        Arc::new(AtomicU64::new(0)),
        path_resolver(),
    )
    .unwrap()
}

fn remove_source(path: &str) -> io::Result<()> {
    fs::remove_file(path)
}

#[test]
fn moves_only_the_first_copy_of_each_name_then_removes_the_owned_pair() {
    let dir = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    let duplicates = tempfile::tempdir().unwrap();
    let audio = source(input.path(), "甲 - 歌.mp3", b"first");
    let lyrics = source(input.path(), "甲 - 歌.lrc", b"[00:01]hello");
    let same_audio = source(duplicates.path(), "甲 - 歌.MP3", b"second");
    let same_lyrics = source(duplicates.path(), "甲 - 歌.LRC", b"[00:01]second");
    let orphan = source(input.path(), "orphan.lrc", b"[00:01]orphan");
    let session_path = dir.path().join("session.json");
    let library = LibraryFiles::new(dir.path().join("installation/assets"));
    let mut player = controller(&session_path);

    let report = player
        .import_managed(
            vec![
                audio.clone(),
                lyrics.clone(),
                same_audio.clone(),
                same_lyrics.clone(),
                orphan.clone(),
            ],
            &library,
            remove_source,
        )
        .unwrap();
    assert_eq!(report.added.len(), 1);
    assert_eq!(
        (
            report.with_lyrics,
            report.orphan_lyrics,
            report.duplicates,
            report.sources_kept
        ),
        (1, 1, 2, 0)
    );
    assert!(!Path::new(&audio.location).exists());
    assert!(!Path::new(&lyrics.location).exists());
    assert!(Path::new(&orphan.location).exists());
    assert!(Path::new(&same_audio.location).exists());
    assert!(Path::new(&same_lyrics.location).exists());

    drop(player);
    let mut player = controller(&session_path);
    let session = load_session(&session_path).unwrap().unwrap();
    let track = &session.tracks[0];
    let audio = Path::new(&track.audio);
    let lyrics = Path::new(track.lyrics.as_deref().unwrap());
    assert!(audio.starts_with(dir.path().join("installation/assets/music")));
    assert!(lyrics.starts_with(dir.path().join("installation/assets/lyrics")));
    assert_eq!(fs::read(audio).unwrap(), b"first");
    assert_eq!(player.lyrics(&report.added[0]).unwrap().lines[0].text, "hello");
    let repeated = player
        .import_managed(vec![same_audio, same_lyrics], &library, remove_source)
        .unwrap();
    assert!(repeated.added.is_empty());
    assert_eq!(repeated.duplicates, 2);

    player.remove_managed(&report.added[0], &library).unwrap();
    assert!(!audio.exists());
    assert!(!lyrics.exists());
    assert!(load_session(&session_path).unwrap().unwrap().tracks.is_empty());
}

#[test]
fn imports_the_sidecar_image_with_its_song_and_removes_them_together() {
    let dir = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    fs::create_dir(input.path().join("img")).unwrap();
    let audio = source(input.path(), "甲 - 歌.mp3", b"first");
    let bare = source(input.path(), "乙 - 曲.mp3", b"second");
    let image_path = input.path().join("img/甲 - 歌.jpg");
    fs::write(&image_path, b"jpeg-bytes").unwrap();
    let session_path = dir.path().join("session.json");
    let library = LibraryFiles::new(dir.path().join("installation/assets"));
    let mut player = controller(&session_path);

    let report = player
        .import_managed(vec![audio, bare], &library, remove_source)
        .unwrap();
    assert_eq!(report.added.len(), 2);

    // 同名图片跟着歌曲进曲库，源图片随源音频一起移走
    let managed_image = dir.path().join("installation/assets/img/甲 - 歌.jpg");
    assert_eq!(fs::read(&managed_image).unwrap(), b"jpeg-bytes");
    assert!(!image_path.exists());
    // 没有同名图片的歌不会带出多余文件
    assert_eq!(
        fs::read_dir(dir.path().join("installation/assets/img"))
            .unwrap()
            .count(),
        1
    );

    player.remove_managed("甲 - 歌", &library).unwrap();
    assert!(!managed_image.exists());
    let session = load_session(&session_path).unwrap().unwrap();
    assert_eq!(session.tracks.len(), 1);
    assert!(Path::new(&session.tracks[0].audio).exists());
    assert!(
        load_session(&session_path).unwrap().unwrap().tracks[0]
            .audio
            .contains("乙 - 曲")
    );
}

#[test]
fn imports_lyrics_and_cover_from_siblings_of_the_music_folder() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input");
    for folder in ["music", "lyrics", "img"] {
        fs::create_dir_all(input.join(folder)).unwrap();
    }
    let audio = source(&input.join("music"), "甲 - 歌.mp3", b"audio");
    let lyrics = source(&input.join("lyrics"), "甲 - 歌.lrc", b"[00:01]hello");
    let image = source(&input.join("img"), "甲 - 歌.jpg", b"jpeg-bytes");
    let root = dir.path().join("assets");
    let library = LibraryFiles::new(root.clone());
    let mut player = controller(&dir.path().join("session.json"));

    let report = player
        .import_managed(vec![audio, lyrics], &library, remove_source)
        .unwrap();

    assert_eq!(report.with_lyrics, 1);
    assert_eq!(player.lyrics(&report.added[0]).unwrap().lines[0].text, "hello");
    assert_eq!(fs::read(root.join("img/甲 - 歌.jpg")).unwrap(), b"jpeg-bytes");
    assert!(!Path::new(&image.location).exists());
}

#[test]
fn removes_a_managed_cover_for_a_legacy_external_song() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("assets");
    fs::create_dir_all(root.join("img")).unwrap();
    let image = root.join("img/甲 - 歌.jpg");
    fs::write(&image, b"image").unwrap();
    let external = source(dir.path(), "甲 - 歌.mp3", b"audio");
    let library = LibraryFiles::new(root);
    let mut player = controller(&dir.path().join("session.json"));
    let id = player.import_files(vec![external.clone()]).added.remove(0);
    assert!(
        music_player_shell_lib::ipc::load_cover_for_track(
            &path_resolver(),
            Path::new(&external.location),
            &library,
            &id,
        )
        .is_some(),
        "id={id:?}"
    );

    player.remove_managed(&id, &library).unwrap();
    assert!(!image.exists());
    assert!(Path::new(&external.location).exists());
}

#[test]
fn restores_deduplicates_and_removes_lyrics_in_the_legacy_owned_folder() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("assets");
    fs::create_dir_all(root.join("music")).unwrap();
    fs::create_dir_all(root.join("lysis")).unwrap();
    let audio = source(&root.join("music"), "song.mp3", b"audio");
    let lyrics = source(&root.join("lysis"), "song.lrc", b"[00:01]old lyrics");
    let duplicate = source(dir.path(), "song.lrc", b"[00:01]duplicate");
    let session_path = dir.path().join("session.json");
    let mut old_player = controller(&session_path);
    let id = old_player.import_files(vec![audio, lyrics.clone()]).added.remove(0);
    old_player.save().unwrap();
    drop(old_player);

    let mut player = controller(&session_path);
    let library = LibraryFiles::new(root);
    assert_eq!(player.lyrics(&id).unwrap().lines[0].text, "old lyrics");
    let report = player
        .import_managed(vec![duplicate.clone()], &library, remove_source)
        .unwrap();
    assert_eq!(report.duplicates, 1);
    player.remove_managed(&id, &library).unwrap();
    assert!(!Path::new(&lyrics.location).exists());
    assert!(Path::new(&duplicate.location).exists());
}

#[test]
fn save_failure_leaves_sources_and_playlist_intact() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session.json");
    let mut player = controller(&session);
    fs::create_dir(&session).unwrap();
    let input = source(dir.path(), "song.mp3", b"audio");
    let root = dir.path().join("assets");
    let library = LibraryFiles::new(root.clone());

    assert!(
        player
            .import_managed(vec![input.clone()], &library, remove_source)
            .is_err()
    );
    assert!(Path::new(&input.location).exists());
    assert!(player.state().tracks.is_empty());
    assert!(fs::read_dir(root.join("music")).unwrap().next().is_none());
}

#[test]
fn android_read_only_source_stays_in_place_and_import_uses_the_local_file() {
    let dir = tempfile::tempdir().unwrap();
    let input = source(dir.path(), "source-no-display-name", b"[00:01]hello");
    let uri = "content://media/document/43";
    let resolver = {
        let input = input.location.clone();
        let open_path = path_resolver();
        Arc::new(move |location: &str| open_path(if location == uri { &input } else { location }))
    };
    let session_path = dir.path().join("session.json");
    let mut player = Controller::new(
        session_path.clone(),
        DeviceBackend::unavailable("test".into()),
        Arc::new(AtomicU64::new(0)),
        resolver,
    )
    .unwrap();
    let legacy_audio = source(dir.path(), "甲 - 歌.mp3", b"audio");
    let id = player.import_files(vec![legacy_audio.clone()]).added.remove(0);
    let library = LibraryFiles::new(dir.path().join("assets"));
    let report = player
        .import_managed(vec![FileRef::new(uri, "甲 - 歌.lrc")], &library, |location| {
            assert_eq!(location, uri);
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        })
        .unwrap();
    assert_eq!(report.lyrics_attached, 1);
    assert_eq!(report.sources_kept, 1);
    let session = load_session(&session_path).unwrap().unwrap();
    let managed_lyrics = Path::new(session.tracks[0].lyrics.as_deref().unwrap());
    assert!(managed_lyrics.exists());
    assert_eq!(player.lyrics(&id).unwrap().lines[0].text, "hello");

    player.remove_managed(&id, &library).unwrap();
    assert!(!managed_lyrics.exists());
    assert!(Path::new(&legacy_audio.location).exists());
    assert!(Path::new(&input.location).exists());
}

#[test]
fn library_only_import_and_removal_work_for_tracks_outside_the_queue() {
    let dir = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    let audio = source(input.path(), "甲 - 歌.mp3", b"audio");
    let session = dir.path().join("session.json");
    let library = LibraryFiles::new(dir.path().join("assets"));
    let mut player = controller(&session);
    let report = player
        .import_managed_with_queue(vec![audio], &library, remove_source, false)
        .unwrap();
    assert!(player.state().tracks.is_empty());
    assert_eq!(player.state().library.len(), 1);
    drop(player);
    let mut player = controller(&session);
    assert_eq!(player.state().library.len(), 1);
    let owned = dir.path().join("assets/music/甲 - 歌.mp3");
    assert!(owned.exists());
    player.remove_managed(&report.added[0], &library).unwrap();
    assert!(!owned.exists());
    assert!(player.state().library.is_empty());
}

#[test]
fn managed_import_saves_the_platform_queue_policy_before_returning() {
    for (behavior, queue_len) in [(PlatformBehavior::desktop(), 1), (PlatformBehavior::android(), 0)] {
        let dir = tempfile::tempdir().unwrap();
        let input = tempfile::tempdir().unwrap();
        let audio = source(input.path(), "甲 - 歌.mp3", b"audio");
        let session = dir.path().join("session.json");
        let library = LibraryFiles::new(dir.path().join("assets"));
        let mut player = controller(&session);
        player.set_behavior(behavior);
        player.import_managed(vec![audio], &library, remove_source).unwrap();

        let saved = load_session(&session).unwrap().unwrap();
        assert_eq!(saved.tracks.len(), queue_len);
        assert_eq!(saved.library.unwrap().len(), 1);
        assert_eq!(player.state().tracks.len(), queue_len);
    }
}
