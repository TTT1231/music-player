#[path = "../src/controller.rs"]
#[allow(dead_code)]
mod controller;

use controller::{AudioBackend, Controller, ControllerEvent, path_resolver};
use music_core::PlayMode;
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
    controller.remove_track(&report.added[0]);
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
    controller.remove_track(&report.added[0]);
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
    controller.remove_track(&report.added[0]);
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
fn cover_url_uses_the_platforms_tauri_protocol_origin() {
    assert_eq!(
        controller::cover_url_for("歌曲 %", true),
        "http://cover.localhost/%E6%AD%8C%E6%9B%B2%20%25"
    );
    assert_eq!(
        controller::cover_url_for("歌曲 %", false),
        "cover://localhost/%E6%AD%8C%E6%9B%B2%20%25"
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
