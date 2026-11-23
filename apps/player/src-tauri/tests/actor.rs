use std::path::PathBuf;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use music_audio::EngineEvent;
use music_player_shell_lib::actor::{Actor, Projection};
use music_player_shell_lib::controller::{AudioBackend, LocationResolver, PlatformBehavior, path_resolver};
use music_player_shell_lib::ipc::{CoverStore, get_cover, get_cover_with, import_files};

struct FakeAudio;
impl AudioBackend for FakeAudio {
    fn load(&mut self, _: &str) -> Result<Duration, String> {
        Ok(Duration::from_secs(3))
    }
    fn play(&mut self) {}
    fn pause(&mut self) {}
    fn seek(&mut self, _: Duration) -> Result<(), String> {
        Ok(())
    }
    fn set_volume(&mut self, _: f32) {}
    fn set_rate(&mut self, _: f32) {}
    fn set_pitch(&mut self, _: i8) {}
    fn position(&self) -> Duration {
        Duration::ZERO
    }
}

#[test]
fn commands_run_on_the_actor_and_shutdown_persists_the_session() {
    let directory = tempfile::tempdir().unwrap();
    let session = directory.path().join("session.json");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/music/No Artist Instrumental.mp3")
        .to_string_lossy()
        .into_owned();
    let (events, received) = mpsc::channel();
    let actor = Actor::spawn(
        session.clone(),
        path_resolver(),
        |_| Ok(FakeAudio),
        move |event| {
            events.send(event).unwrap();
        },
    )
    .unwrap();
    let imported = actor
        .call(move |controller| controller.import_paths(&[fixture]))
        .unwrap()
        .unwrap();
    assert_eq!(imported.added.len(), 1);
    let import_json = serde_json::to_value(&imported).unwrap();
    assert_eq!(import_json["withLyrics"], 0);
    assert_eq!(import_json["lyricsAttached"], 0);
    assert_eq!(import_json["orphanLyrics"], 0);
    let state = actor.call(|controller| controller.state()).unwrap();
    assert_eq!(state.tracks.len(), 1);
    let serialized = serde_json::to_value(&state).unwrap();
    assert_eq!(serialized["currentId"], serde_json::Value::Null);
    assert_eq!(serialized["positionMs"], 0);
    assert_eq!(serialized["tracks"][0]["hasLyrics"], false);
    assert!(
        serialized["tracks"][0]["coverUrl"]
            .as_str()
            .unwrap()
            .starts_with("http://cover.localhost/")
    );
    assert!(matches!(
        received.recv_timeout(Duration::from_secs(1)).unwrap(),
        Projection::State(_)
    ));
    actor.shutdown();
    assert!(session.exists());
}

#[test]
fn queued_end_from_previous_song_does_not_advance_the_new_song() {
    let directory = tempfile::tempdir().unwrap();
    let (callback_tx, callback_rx) = mpsc::channel();
    let actor = Actor::spawn(
        directory.path().join("session.json"),
        path_resolver(),
        move |callback| {
            callback_tx.send(callback).unwrap();
            Ok(FakeAudio)
        },
        |_| {},
    )
    .unwrap();
    let callback = callback_rx.recv().unwrap();
    let fixture = |name: &str| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../fixtures/music")
            .join(name)
            .to_string_lossy()
            .into_owned()
    };
    let paths = vec![
        fixture("No Artist Instrumental.mp3"),
        fixture("Test Artist - Track Two.ogg"),
    ];
    let ids = actor.call(move |c| c.import_paths(&paths)).unwrap().unwrap().added;
    let first = ids[0].clone();
    let second = ids[1].clone();
    actor.call(move |c| c.play_track(&first)).unwrap();
    let expected = second.clone();
    actor
        .call(move |c| {
            callback(EngineEvent::Ended);
            c.play_track(&second);
        })
        .unwrap();
    let state = actor.call(|c| c.state()).unwrap();
    assert_eq!(state.current_id.as_deref(), Some(expected.as_str()));
    actor.shutdown();
}

#[test]
fn cover_is_read_through_the_location_resolver_for_content_uris() {
    let directory = tempfile::tempdir().unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/music/Test Artist、Guest - Track One (Live).mp3");
    // 模拟安卓：content:// URI 不是路径，只能经解析器打开
    let resolver: LocationResolver = Arc::new(move |location| {
        assert_eq!(location, "content://media/document/audio%3A1");
        Ok((std::fs::File::open(&fixture)?, None))
    });
    let actor = Actor::spawn(
        directory.path().join("session.json"),
        resolver,
        |_| Ok(FakeAudio),
        |_| {},
    )
    .unwrap();
    let file = music_core::FileRef::new(
        "content://media/document/audio%3A1",
        "Test Artist、Guest - Track One (Live).mp3",
    );
    let id = actor.call(move |c| c.import_files(vec![file])).unwrap().added.remove(0);
    let covers = CoverStore::default();
    let library = music_player_shell_lib::library_files::LibraryFiles::new(directory.path().join("assets"));
    let cover = get_cover(&actor, &covers, &library, id).expect("cover read via resolver");
    assert_eq!(cover.mime, "image/png");
    actor.shutdown();
}

#[test]
fn importing_a_replacement_invalidates_cached_cover_even_at_the_same_path() {
    let directory = tempfile::tempdir().unwrap();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/music");
    let path = directory.path().join("song.mp3");
    std::fs::copy(fixtures.join("Test Artist、Guest - Track One (Live).mp3"), &path).unwrap();
    let actor = Actor::spawn(
        directory.path().join("session.json"),
        path_resolver(),
        |_| Ok(FakeAudio),
        |_| {},
    )
    .unwrap();
    let covers = CoverStore::default();
    let library = music_player_shell_lib::library_files::LibraryFiles::new(directory.path().join("assets"));
    let paths = vec![path.to_string_lossy().into_owned()];
    let id = import_files(&actor, &covers, paths.clone()).unwrap().added.remove(0);
    assert!(get_cover(&actor, &covers, &library, id.clone()).is_some());
    std::fs::copy(fixtures.join("No Artist Instrumental.mp3"), &path).unwrap();
    import_files(&actor, &covers, paths).unwrap();
    assert!(get_cover(&actor, &covers, &library, id).is_none());
    actor.shutdown();
}

#[test]
fn slow_cover_probe_does_not_block_import_or_restore_an_old_cover() {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    let directory = tempfile::tempdir().unwrap();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/music");
    let path = directory.path().join("song.mp3");
    std::fs::copy(fixtures.join("Test Artist、Guest - Track One (Live).mp3"), &path).unwrap();
    let actor = Arc::new(
        Actor::spawn(
            directory.path().join("session.json"),
            path_resolver(),
            |_| Ok(FakeAudio),
            |_| {},
        )
        .unwrap(),
    );
    let covers = Arc::new(CoverStore::default());
    let paths = vec![path.to_string_lossy().into_owned()];
    let id = import_files(&actor, &covers, paths.clone()).unwrap().added.remove(0);
    let old_cover = music_audio::probe(&path).unwrap().cover.unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    let count = Arc::new(AtomicUsize::new(0));
    let reader = {
        let actor = Arc::clone(&actor);
        let covers = Arc::clone(&covers);
        let count = Arc::clone(&count);
        std::thread::spawn(move || {
            get_cover_with(&actor, &covers, id, |path| {
                if count.fetch_add(1, Ordering::SeqCst) == 0 {
                    started_tx.send(()).unwrap();
                    release_rx.lock().unwrap().recv().unwrap();
                    Some(old_cover.clone())
                } else {
                    music_audio::probe(path).ok().and_then(|info| info.cover)
                }
            })
        })
    };
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    std::fs::copy(fixtures.join("No Artist Instrumental.mp3"), &path).unwrap();
    let (finished_tx, finished_rx) = mpsc::channel();
    let importer = {
        let actor = Arc::clone(&actor);
        let covers = Arc::clone(&covers);
        std::thread::spawn(move || {
            finished_tx.send(import_files(&actor, &covers, paths)).unwrap();
        })
    };
    let result = finished_rx.recv_timeout(Duration::from_secs(2));
    release_tx.send(()).unwrap();
    importer.join().unwrap();
    assert!(result.unwrap().is_ok(), "封面读取不应阻塞导入");
    assert!(reader.join().unwrap().is_none(), "过期封面不能写回");
    actor.shutdown();
}

#[test]
fn library_changes_are_projected_as_their_own_event_only_where_the_ui_reads_them() {
    let directory = tempfile::tempdir().unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/music/No Artist Instrumental.mp3")
        .to_string_lossy()
        .into_owned();
    let (events, received) = mpsc::channel();
    let actor = Actor::spawn(
        directory.path().join("session.json"),
        path_resolver(),
        |_| Ok(FakeAudio),
        move |event| {
            events.send(event).unwrap();
        },
    )
    .unwrap();
    actor.call(move |c| c.import_paths(&[fixture])).unwrap().unwrap();
    assert!(matches!(
        received.recv_timeout(Duration::from_secs(1)).unwrap(),
        Projection::State(_)
    ));
    // 等时长探测完成：它会让播放状态再投影一次
    let deadline = Instant::now() + Duration::from_secs(5);
    while !actor.call(|c| c.state().tracks[0].duration_ms.is_some()).unwrap() {
        assert!(Instant::now() < deadline, "时长没有探测出来");
        std::thread::sleep(Duration::from_millis(20));
    }
    // 桌面界面只读播放状态：导入和时长都没有另外投影成 library 事件
    while let Ok(event) = received.try_recv() {
        assert!(matches!(event, Projection::State(_)), "桌面不应收到 {event:?}");
    }

    actor.call(|c| c.set_behavior(PlatformBehavior::android())).unwrap();
    let night = actor.call(|c| c.create_playlist("夜跑")).unwrap().unwrap();
    let Projection::Library(library) = received.recv_timeout(Duration::from_secs(1)).unwrap() else {
        panic!("歌单变化应投影为 library 事件");
    };
    assert_eq!(library.playlists[0].id, night);
    assert_eq!(library.playlists[0].name, "夜跑");
    assert_eq!(library.tracks.len(), 1);
    // 没有播放状态变化时不发多余的 state 事件
    assert!(received.recv_timeout(Duration::from_millis(300)).is_err());
    actor.shutdown();
}
