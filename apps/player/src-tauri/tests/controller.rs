#[path = "../src/controller.rs"]
#[allow(dead_code)]
mod controller;

use controller::{AudioBackend, Controller, ControllerEvent};
use music_core::PlayMode;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, atomic::AtomicU64};
use std::time::Duration;

#[derive(Default)]
struct BackendState {
    loaded: Vec<PathBuf>,
    playing: bool,
    position: Duration,
    unreadable: Vec<PathBuf>,
    unavailable: bool,
}
struct FakeBackend(Arc<Mutex<BackendState>>);
impl AudioBackend for FakeBackend {
    fn available(&self) -> bool {
        !self.0.lock().unwrap().unavailable
    }
    fn load(&mut self, path: &Path) -> Result<Duration, String> {
        let mut state = self.0.lock().unwrap();
        state.loaded.push(path.to_owned());
        if state.unreadable.contains(&path.to_owned()) {
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
    )
    .unwrap();
    (controller, backend)
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
    backend.lock().unwrap().unreadable = paths.iter().map(PathBuf::from).collect();
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
    assert_eq!(backend.lock().unwrap().loaded.last(), Some(&replacement));
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
    backend.lock().unwrap().unreadable.push(PathBuf::from(&paths[0]));
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
