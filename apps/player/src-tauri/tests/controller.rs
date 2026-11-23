use music_core::PlayMode;
use music_player_shell_lib::controller::{
    AudioBackend, Controller, ControllerEvent, PlatformBehavior, TrackView, path_resolver,
};
use music_player_shell_lib::library_files::LibraryFiles;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, atomic::AtomicU64};
use std::time::Duration;

#[derive(Default)]
struct BackendState {
    loaded: Vec<String>,
    playing: bool,
    position: Duration,
    unreadable: Vec<String>,
    unavailable: bool,
    /// 设备丢失后能否重新打开输出。
    recoverable: bool,
    recoveries: usize,
    release_when_paused: bool,
}
struct FakeBackend(Arc<Mutex<BackendState>>);
impl AudioBackend for FakeBackend {
    fn available(&self) -> bool {
        !self.0.lock().unwrap().unavailable
    }
    fn recover(&mut self) -> bool {
        let mut state = self.0.lock().unwrap();
        state.recoveries += 1;
        if state.recoverable {
            state.unavailable = false;
            state.playing = false;
            state.position = Duration::ZERO;
        }
        state.recoverable
    }
    fn release_idle(&mut self) -> bool {
        let mut state = self.0.lock().unwrap();
        if state.release_when_paused && !state.unavailable {
            state.unavailable = true;
            return true;
        }
        false
    }
    fn load(&mut self, location: &str) -> Result<Duration, String> {
        let mut state = self.0.lock().unwrap();
        state.loaded.push(location.to_owned());
        if state.unreadable.contains(&location.to_owned()) {
            Err("unreadable".into())
        } else {
            Ok(Duration::from_secs(3))
        }
    }
    fn play(&mut self) {
        self.0.lock().unwrap().playing = true;
    }
    fn pause(&mut self) {
        self.0.lock().unwrap().playing = false;
    }
    fn seek(&mut self, position: Duration) -> Result<(), String> {
        self.0.lock().unwrap().position = position;
        Ok(())
    }
    fn set_volume(&mut self, _: f32) {}
    fn set_rate(&mut self, _: f32) {}
    fn set_pitch(&mut self, _: i8) {}
    fn position(&self) -> Duration {
        self.0.lock().unwrap().position
    }
}
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/music")
        .join(name)
}
fn paths() -> Vec<String> {
    [
        "No Artist Instrumental.mp3",
        "Test Artist - Track Two.ogg",
        "subfolder/Test Artist - Track Three.ogg",
    ]
    .iter()
    .map(|name| fixture(name).to_string_lossy().into_owned())
    .collect()
}
fn setup(path: &Path) -> (Controller<FakeBackend>, Arc<Mutex<BackendState>>) {
    let backend = Arc::new(Mutex::new(BackendState::default()));
    let controller = Controller::new(
        path.to_owned(),
        FakeBackend(backend.clone()),
        Arc::new(AtomicU64::new(0)),
        path_resolver(),
    )
    .unwrap();
    (controller, backend)
}

#[test]
fn content_uris_use_display_names_for_titles_and_lyric_pairing() {
    // 安卓内容 URI 不含文件名：歌名和歌词配对必须来自系统文件选择器给出的显示名
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, _backend) = setup(&dir.path().join("session.json"));
    let file = |id: &str, name: &str| music_core::FileRef::new(format!("content://media/document/{id}"), name);
    let report = controller.import_files(vec![
        file("audio%3A12", "歌手 - 歌名.mp3"),
        file("document%3A40", "歌手 - 歌名.lrc"),
    ]);
    assert_eq!(report.added.len(), 1);
    assert_eq!(report.with_lyrics, 1);
    let track = &controller.state().tracks[0];
    assert_eq!((track.title.as_str(), track.artist.as_str()), ("歌名", "歌手"));
    assert!(track.has_lyrics);
}

#[test]
fn imports_plays_and_uses_mode_when_track_ends() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, backend) = setup(&dir.path().join("session.json"));
    let report = controller.import_paths(&paths()).unwrap();
    assert_eq!(report.added.len(), 3);
    assert_eq!(controller.state().tracks.len(), 3);
    controller.play_track(&report.added[0]);
    assert!(controller.state().playing);
    controller.set_mode(PlayMode::Order);
    controller.on_ended();
    assert_eq!(controller.state().current_id.as_deref(), Some(report.added[1].as_str()));
    assert_eq!(backend.lock().unwrap().loaded.len(), 2);
}

#[test]
fn skips_unreadable_files_and_stops_after_each_track_fails() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, backend) = setup(&dir.path().join("session.json"));
    let paths = paths();
    let report = controller.import_paths(&paths).unwrap();
    backend.lock().unwrap().unreadable = paths.iter().map(String::as_str).map(str::to_owned).collect();
    controller.play_track(&report.added[0]);
    let state = controller.state();
    assert!(!state.playing);
    assert_eq!(backend.lock().unwrap().loaded.len(), 3);
    let events = controller.poll().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, ControllerEvent::Notice { .. }))
            .count(),
        3
    );
}

#[test]
fn removing_current_switches_to_next_and_session_restores_paused() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session.json");
    let (mut controller, _) = setup(&session);
    let report = controller.import_paths(&paths()).unwrap();
    controller.play_track(&report.added[0]);
    controller.remove_queue_track(&report.added[0]);
    controller.set_mode(PlayMode::Single);
    controller.set_volume(0.4);
    assert_eq!(controller.state().current_id.as_deref(), Some(report.added[1].as_str()));
    assert!(controller.state().playing);
    controller.save().unwrap();
    drop(controller);
    let (restored, _) = setup(&session);
    let state = restored.state();
    assert_eq!(state.tracks.len(), 2);
    assert_eq!(state.current_id.as_deref(), Some(report.added[1].as_str()));
    assert!(!state.playing);
    assert_eq!(state.mode, PlayMode::Single);
    assert!((state.volume - 0.4).abs() < 0.001);
    assert!(Path::new(&paths()[0]).exists());
}

#[test]
fn restored_track_can_seek_before_first_play() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session.json");
    let (mut controller, _) = setup(&session);
    let report = controller.import_paths(&paths()).unwrap();
    controller.play_track(&report.added[0]);
    controller.pause();
    controller.save().unwrap();
    drop(controller);
    let (mut restored, backend) = setup(&session);
    restored.seek(1_500).unwrap();
    assert_eq!(backend.lock().unwrap().loaded.len(), 1);
    assert_eq!(restored.state().position_ms, 1_500);
    assert!(!restored.state().playing);
    assert!(restored.poll().unwrap().contains(&ControllerEvent::StateChanged));
    restored.play();
    assert_eq!(restored.state().position_ms, 1_500);
}

#[test]
fn replacing_playing_track_reloads_new_file() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, backend) = setup(&dir.path().join("session.json"));
    let original = paths()[0].clone();
    let report = controller.import_paths(std::slice::from_ref(&original)).unwrap();
    controller.play_track(&report.added[0]);
    let replacement = dir.path().join(Path::new(&original).file_name().unwrap());
    std::fs::copy(&original, &replacement).unwrap();
    controller
        .import_paths(&[replacement.to_string_lossy().into_owned()])
        .unwrap();
    assert_eq!(
        backend.lock().unwrap().loaded.last(),
        Some(&replacement.to_string_lossy().into_owned())
    );
    assert!(controller.state().playing);
}

#[test]
fn removing_paused_current_keeps_next_paused_and_save_is_debounced() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session.json");
    let (mut controller, backend) = setup(&session);
    let report = controller.import_paths(&paths()).unwrap();
    controller.play_track(&report.added[0]);
    controller.pause();
    controller.remove_queue_track(&report.added[0]);
    assert_eq!(controller.state().current_id.as_deref(), Some(report.added[1].as_str()));
    assert!(!controller.state().playing);
    assert!(!backend.lock().unwrap().playing);
    assert!(!session.exists());
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while !session.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
        controller.poll().unwrap();
    }
    assert!(session.exists());
    drop(controller);
    let (restored, _) = setup(&session);
    assert_eq!(restored.state().current_id.as_deref(), Some(report.added[1].as_str()));
}

#[test]
fn late_end_event_does_not_resume_a_paused_player() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, _) = setup(&dir.path().join("session.json"));
    let report = controller.import_paths(&paths()).unwrap();
    controller.play_track(&report.added[0]);
    controller.pause();
    controller.on_ended();
    assert!(!controller.state().playing);
    assert_eq!(controller.state().current_id.as_deref(), Some(report.added[0].as_str()));
}

#[test]
fn seeking_unreadable_restored_track_persists_skipped_track() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session.json");
    let (mut controller, _) = setup(&session);
    let paths = paths();
    let report = controller.import_paths(&paths).unwrap();
    controller.play_track(&report.added[0]);
    controller.pause();
    controller.save().unwrap();
    drop(controller);
    let (mut restored, backend) = setup(&session);
    backend.lock().unwrap().unreadable.push(paths[0].clone());
    restored.seek(1_000).unwrap();
    assert_eq!(restored.state().current_id.as_deref(), Some(report.added[1].as_str()));
    drop(restored);
    let (restored_again, _) = setup(&session);
    assert_eq!(
        restored_again.state().current_id.as_deref(),
        Some(report.added[1].as_str())
    );
}

#[test]
fn pause_invalidates_queued_device_events_even_after_resume() {
    use std::sync::atomic::Ordering;
    let dir = tempfile::tempdir().unwrap();
    let backend = Arc::new(Mutex::new(BackendState::default()));
    let generation = Arc::new(AtomicU64::new(0));
    let mut controller = Controller::new(
        dir.path().join("session.json"),
        FakeBackend(backend),
        generation.clone(),
        path_resolver(),
    )
    .unwrap();
    let report = controller.import_paths(&paths()).unwrap();
    controller.play_track(&report.added[0]);
    let queued_generation = generation.load(Ordering::SeqCst);
    controller.pause();
    controller.play();
    assert_ne!(queued_generation, generation.load(Ordering::SeqCst));
}

#[test]
fn missing_output_device_keeps_selected_track_without_unreadable_notices() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, backend) = setup(&dir.path().join("session.json"));
    let report = controller.import_paths(&paths()).unwrap();
    backend.lock().unwrap().unavailable = true;
    controller.play_track(&report.added[0]);
    assert_eq!(controller.state().current_id.as_deref(), Some(report.added[0].as_str()));
    assert!(!controller.state().playing);
    assert!(backend.lock().unwrap().loaded.is_empty());
    assert!(
        !controller
            .poll()
            .unwrap()
            .iter()
            .any(|e| matches!(e, ControllerEvent::Notice { .. }))
    );
    controller.remove_queue_track(&report.added[0]);
    assert_eq!(controller.state().current_id.as_deref(), Some(report.added[1].as_str()));
    assert!(!controller.state().playing);
    assert!(backend.lock().unwrap().loaded.is_empty());
}

#[test]
fn unavailable_backend_cannot_leave_previous_playback_running() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, backend) = setup(&dir.path().join("session.json"));
    let report = controller.import_paths(&paths()).unwrap();
    controller.play_track(&report.added[0]);
    backend.lock().unwrap().unavailable = true;
    controller.play();
    assert!(!backend.lock().unwrap().playing);
    assert!(!controller.state().playing);
}

#[test]
fn lost_output_device_is_reopened_and_resumes_the_same_song_at_the_same_position() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, backend) = setup(&dir.path().join("session.json"));
    let report = controller.import_paths(&paths()).unwrap();
    controller.play_track(&report.added[1]);
    controller.seek(1500).unwrap();
    controller.on_device_error("The requested device is no longer available.".into());
    {
        let mut state = backend.lock().unwrap();
        state.unavailable = true;
        state.recoverable = true;
        state.loaded.clear();
    }
    controller.recover_output();
    controller.play();
    let state = backend.lock().unwrap();
    assert_eq!(state.recoveries, 1);
    assert_eq!(
        state.loaded,
        vec![paths()[1].clone()],
        "重新打开设备后要重新载入当前歌曲"
    );
    assert_eq!(state.position, Duration::from_millis(1500), "从中断的位置继续");
    assert!(state.playing);
    drop(state);
    assert!(controller.state().playing);
    assert_eq!(controller.state().current_id.as_deref(), Some(report.added[1].as_str()));
}

#[test]
fn paused_player_releases_idle_output_and_resumes_at_its_previous_position() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, backend) = setup(&dir.path().join("session.json"));
    let report = controller.import_paths(&paths()).unwrap();
    {
        let mut state = backend.lock().unwrap();
        state.release_when_paused = true;
        state.recoverable = true;
    }
    controller.play_track(&report.added[0]);
    controller.seek(700).unwrap();
    controller.poll().unwrap();
    assert!(!backend.lock().unwrap().unavailable);
    controller.pause();
    controller.poll().unwrap();
    assert!(backend.lock().unwrap().unavailable);
    assert_eq!(controller.state().position_ms, 700);

    controller.recover_output();
    controller.play();
    let state = controller.state();
    assert!(state.playing);
    assert_eq!(state.position_ms, 700);
    assert_eq!(state.current_id.as_deref(), Some(report.added[0].as_str()));
}

#[test]
fn recovery_is_skipped_while_available_and_failure_keeps_playback_stopped() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, backend) = setup(&dir.path().join("session.json"));
    let report = controller.import_paths(&paths()).unwrap();
    controller.play_track(&report.added[0]);
    controller.recover_output();
    assert_eq!(backend.lock().unwrap().recoveries, 0, "设备正常时不重开");
    controller.on_device_error("lost".into());
    backend.lock().unwrap().unavailable = true;
    controller.recover_output();
    controller.play();
    assert_eq!(backend.lock().unwrap().recoveries, 1);
    assert!(!backend.lock().unwrap().playing);
    assert!(!controller.state().playing);
}

#[test]
fn cover_url_encodes_the_track_id_for_the_platform_protocol() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, _) = setup(&dir.path().join("session.json"));
    controller.import_files(vec![music_core::FileRef::new("song.mp3", "歌曲 %.mp3")]);
    let origin = if cfg!(any(target_os = "windows", target_os = "android")) {
        "http://cover.localhost/"
    } else {
        "cover://localhost/"
    };
    assert_eq!(
        controller.state().tracks[0].cover_url,
        format!("{origin}%E6%AD%8C%E6%9B%B2%20%25")
    );
}

#[test]
fn imports_saf_uris_by_treating_location_verbatim() {
    // 安卓 SAF：位置是 content:// URI 而非路径；按原样记录，歌名从末段文件名解析
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, backend) = setup(&dir.path().join("session.json"));
    // 外置存储 provider 的 document id 是未编码的相对路径（SAF 常见形式），末段即文件名
    let uri =
        "content://com.android.externalstorage.documents/document/primary:Music/Test Artist - Track Two.ogg".to_owned();
    let report = controller.import_paths(std::slice::from_ref(&uri)).unwrap();
    assert_eq!(report.added.len(), 1, "URI 应作为一首歌导入");
    let state = controller.state();
    assert_eq!(state.tracks.len(), 1);
    assert_eq!(state.tracks[0].title, "Track Two");
    assert_eq!(state.tracks[0].artist, "Test Artist");
    // 位置按原样交给音频后端（URI 透明传递）
    controller.play_track(&report.added[0]);
    assert_eq!(backend.lock().unwrap().loaded, vec![uri]);
}

#[test]
fn playlist_commands_keep_library_queue_and_saved_selection_separate_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session.json");
    let (mut controller, _) = setup(&session);
    let ids = controller.import_paths(&paths()).unwrap().added;
    let list = controller.create_playlist("收藏").unwrap();
    controller
        .add_playlist_tracks(&list, &[ids[1].clone(), ids[0].clone()])
        .unwrap();
    controller.play_playlist(Some(&list), Some(&ids[1])).unwrap();
    assert_eq!(controller.state().tracks.len(), 2);
    controller.remove_queue_track(&ids[1]);
    assert_eq!(controller.state().library.len(), 3);
    assert_eq!(controller.state().playlists[0].track_ids.len(), 2);
    controller.save().unwrap();
    drop(controller);
    let (restored, _) = setup(&session);
    assert_eq!(restored.state().tracks.len(), 1);
    assert_eq!(restored.state().library.len(), 3);
    assert_eq!(restored.state().playlists[0].track_ids.len(), 2);
    assert_eq!(restored.state().queue_source.as_deref(), Some(list.as_str()));
}

/// 安卓：导入只进曲库，播放列表在用户从歌单点播时整份替换。
fn android(session: &Path) -> (Controller<FakeBackend>, Arc<Mutex<BackendState>>) {
    let (mut controller, backend) = setup(session);
    controller.set_behavior(PlatformBehavior::android());
    (controller, backend)
}

fn ids(tracks: &[TrackView]) -> Vec<&str> {
    tracks.iter().map(|track| track.id.as_str()).collect()
}

#[test]
fn desktop_imports_join_the_queue_and_only_playback_state_is_projected() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, _) = setup(&dir.path().join("session.json"));
    let report = controller.import_paths(&paths()).unwrap();
    // 桌面沿用：导入的歌直接排进播放列表，界面只读播放状态
    assert_eq!(ids(&controller.state().tracks), report.added);
    assert_eq!(ids(&controller.library_state().tracks), report.added);
    let events = controller.poll().unwrap();
    assert!(events.contains(&ControllerEvent::StateChanged));
    assert!(!events.contains(&ControllerEvent::LibraryChanged));
}

#[test]
fn android_imports_only_reach_the_library_and_playing_from_a_list_replaces_the_queue() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, backend) = android(&dir.path().join("session.json"));
    let report = controller.import_paths(&paths()).unwrap();
    assert_eq!(ids(&controller.library_state().tracks), report.added);
    assert!(controller.state().tracks.is_empty(), "导入不进播放列表");
    assert!(backend.lock().unwrap().loaded.is_empty());
    let events = controller.poll().unwrap();
    assert!(events.contains(&ControllerEvent::LibraryChanged));

    // 从「全部音乐」点第二首：播放列表换成给出的顺序，从点中的歌开始
    controller.set_mode(PlayMode::Order);
    controller.play_playlist(None, Some(&report.added[1])).unwrap();
    assert_eq!(controller.state().queue_source.as_deref(), Some("all"));
    assert_eq!(ids(&controller.state().tracks), report.added);
    assert_eq!(controller.state().current_id.as_deref(), Some(report.added[1].as_str()));
    assert_eq!(backend.lock().unwrap().loaded, vec![paths()[1].clone()]);
    controller.on_ended();
    assert_eq!(controller.state().current_id.as_deref(), Some(report.added[2].as_str()));

    // 起点不在给出的列表里：什么都不变
    assert!(!controller.play_from(&report.added, "不存在"));
    assert_eq!(controller.state().current_id.as_deref(), Some(report.added[2].as_str()));

    // 之后导入的歌只进曲库，不插进正在用的播放列表
    let extra = fixture("No Artist Instrumental.mp3");
    let added = controller
        .import_files(vec![music_core::FileRef::new(
            extra.to_string_lossy().into_owned(),
            "歌手 - 新歌.mp3",
        )])
        .added;
    assert_eq!(added.len(), 1);
    assert_eq!(controller.library_state().tracks.len(), 4);
    assert_eq!(ids(&controller.state().tracks), report.added);
}

#[test]
fn removing_from_the_queue_keeps_the_library_and_playlists() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, _) = setup(&dir.path().join("session.json"));
    let report = controller.import_paths(&paths()).unwrap();
    let night = controller.create_playlist("夜跑").unwrap();
    controller.add_playlist_tracks(&night, &report.added).unwrap();
    controller.play_track(&report.added[0]);

    controller.remove_queue_track(&report.added[0]);

    // 只移出播放列表：接着播排在后面的歌；曲库、歌单和文件都不动
    let state = controller.state();
    assert_eq!(state.current_id.as_deref(), Some(report.added[1].as_str()));
    assert_eq!(ids(&state.tracks), report.added[1..]);
    assert_eq!(ids(&controller.library_state().tracks), report.added);
    assert_eq!(controller.library_state().playlists[0].track_ids, report.added);
    assert!(Path::new(&paths()[0]).exists());
}

#[test]
fn removing_from_the_library_clears_the_queue_and_playlists() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, _) = android(&dir.path().join("session.json"));
    let report = controller.import_paths(&paths()).unwrap();
    controller.play_from(&report.added, &report.added[1]);
    let night = controller.create_playlist("夜跑").unwrap();
    controller.add_playlist_tracks(&night, &report.added).unwrap();

    controller
        .remove_managed(&report.added[1], &LibraryFiles::new(dir.path().join("assets")))
        .unwrap();

    let state = controller.state();
    assert_eq!(
        state.current_id.as_deref(),
        Some(report.added[2].as_str()),
        "接着播列表里排在后面的歌"
    );
    let remaining = [report.added[0].as_str(), report.added[2].as_str()];
    assert_eq!(ids(&state.tracks), remaining);
    let library = controller.library_state();
    assert_eq!(ids(&library.tracks), remaining);
    assert_eq!(library.playlists[0].track_ids, remaining);
    let events = controller.poll().unwrap();
    assert!(events.contains(&ControllerEvent::StateChanged));
    assert!(events.contains(&ControllerEvent::LibraryChanged));
    // 这里的文件不在曲库目录里（相当于旧会话的外部文件），留在原处
    assert!(Path::new(&paths()[1]).exists());
}

#[test]
fn playlist_edits_are_projected_on_their_own_and_persisted() {
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("session.json");
    let (mut controller, _) = android(&session);
    let report = controller.import_paths(&paths()).unwrap();
    controller.poll().unwrap();

    let night = controller.create_playlist(" 夜跑 ").unwrap();
    assert_eq!(controller.add_playlist_tracks(&night, &report.added[..2]).unwrap(), 2);
    assert_eq!(
        controller
            .add_playlist_tracks(&night, &[report.added[0].clone(), "不存在".to_owned()])
            .unwrap(),
        0
    );
    let library = controller.library_state();
    assert_eq!(library.playlists[0].id, night);
    assert_eq!(library.playlists[0].name, "夜跑");
    assert_eq!(library.playlists[0].track_ids, report.added[..2]);
    // 歌单操作只投影曲库，不打扰播放状态
    let events = controller.poll().unwrap();
    assert!(!events.is_empty());
    assert!(
        events
            .iter()
            .all(|event| matches!(event, ControllerEvent::LibraryChanged))
    );

    // 重名和内置名称被拒绝，错误带能直接显示的文字
    assert_eq!(
        controller.create_playlist("夜跑").unwrap_err().to_string(),
        "已经有同名的歌单了"
    );
    assert_eq!(
        controller.create_playlist("全部音乐").unwrap_err().to_string(),
        "「全部音乐」是内置歌单，请换一个名字"
    );
    assert!(controller.rename_playlist(&night, "全部音乐").is_err());
    controller.rename_playlist(&night, " 深夜跑 ").unwrap();
    assert_eq!(controller.library_state().playlists[0].name, "深夜跑");

    controller.remove_playlist_track(&night, &report.added[1]).unwrap();
    assert_eq!(controller.library_state().playlists[0].track_ids, report.added[..1]);
    controller.save().unwrap();
    drop(controller);

    let (mut restored, _) = android(&session);
    let library = restored.library_state();
    assert_eq!(library.playlists.len(), 1);
    assert_eq!(library.playlists[0].name, "深夜跑");
    assert_eq!(library.playlists[0].track_ids, report.added[..1]);
    assert_eq!(ids(&library.tracks), report.added);
    assert!(restored.state().tracks.is_empty(), "空着的播放列表重启后仍是空的");

    // 删除歌单不动曲库
    restored.delete_playlist(&night).unwrap();
    let library = restored.library_state();
    assert!(library.playlists.is_empty());
    assert_eq!(ids(&library.tracks), report.added);
    assert_eq!(restored.delete_playlist(&night).unwrap_err().to_string(), "歌单不存在");
}

#[test]
fn probed_durations_reach_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, _) = android(&dir.path().join("session.json"));
    controller.import_paths(&paths()).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut projected = false;
    while controller
        .library_state()
        .tracks
        .iter()
        .any(|track| track.duration_ms.is_none())
    {
        assert!(std::time::Instant::now() < deadline, "时长没有探测出来");
        std::thread::sleep(Duration::from_millis(50));
        projected |= controller.poll().unwrap().contains(&ControllerEvent::LibraryChanged);
    }
    assert!(projected, "探测到时长后要重新投影曲库，列表里才显示时长");
}

#[test]
fn loading_a_track_projects_its_duration_to_the_android_library() {
    let dir = tempfile::tempdir().unwrap();
    let (mut controller, _) = android(&dir.path().join("session.json"));
    let file = music_core::FileRef::new("missing.ogg", "歌手 - 歌.ogg");
    let report = controller.import_files(vec![file]);
    controller.poll().unwrap();
    assert_eq!(controller.library_state().tracks[0].duration_ms, None);

    controller.play_playlist(None, Some(&report.added[0])).unwrap();
    assert!(controller.library_state().tracks[0].duration_ms.is_some());
    assert!(controller.poll().unwrap().contains(&ControllerEvent::LibraryChanged));
}
