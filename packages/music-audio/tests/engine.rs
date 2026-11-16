mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{OGG_NO_COVER, fixture};
use music_audio::{AudioSink, Engine, EngineEvent, decode_file};

#[derive(Clone)]
struct FakeSink {
    samples: Arc<Mutex<Vec<f32>>>,
}

impl AudioSink for FakeSink {
    fn sample_rate(&self) -> u32 {
        44_100
    }
    fn channels(&self) -> u16 {
        2
    }
    fn write(&mut self, samples: &[f32]) {
        self.samples.lock().unwrap().extend_from_slice(samples);
    }
}

type SharedSamples = Arc<Mutex<Vec<f32>>>;
type SharedEvents = Arc<Mutex<Vec<EngineEvent>>>;

fn engine() -> (Engine<FakeSink>, SharedSamples, SharedEvents) {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let events = Arc::new(Mutex::new(Vec::new()));
    let received = events.clone();
    let mut engine = Engine::new(
        FakeSink {
            samples: samples.clone(),
        },
        move |event| {
            received.lock().unwrap().push(event);
        },
    );
    engine.load(&fixture(OGG_NO_COVER)).unwrap();
    (engine, samples, events)
}

#[test]
fn ends_after_sink_pulls_the_whole_track() {
    let (mut engine, _, events) = engine();
    engine.play();
    for _ in 0..140 {
        engine.pull(1024);
    }
    assert_eq!(*events.lock().unwrap(), vec![EngineEvent::Ended]);
    assert!((engine.position().as_secs_f64() - 3.0).abs() < 0.1);
}

#[test]
fn position_follows_audible_input_at_changed_rate_and_pause() {
    let (mut engine, _, _) = engine();
    engine.set_rate(1.5);
    engine.play();
    engine.pull(22_050);
    assert!((engine.position().as_secs_f64() - 0.75).abs() < 0.04);
    engine.pause();
    let before = engine.position();
    engine.pull(4_410);
    assert_eq!(engine.position(), before);
}

#[test]
fn volume_scales_samples_and_seek_starts_at_new_position() {
    let (mut engine, samples, _) = engine();
    engine.set_volume(0.5);
    engine.play();
    engine.pull(4_410);
    let reference = decode_file(&fixture(OGG_NO_COVER)).unwrap();
    let actual = samples.lock().unwrap();
    assert!(
        actual
            .as_chunks::<2>()
            .0
            .iter()
            .zip(&reference.samples)
            .all(|(frame, b)| frame.iter().all(|a| (a - b * 0.5).abs() < 1e-6))
    );
    drop(actual);
    engine.seek(Duration::from_millis(1500)).unwrap();
    engine.pull(4_410);
    assert!((engine.position().as_secs_f64() - 1.6).abs() < 0.05);
}

/// 输出里最长的一段连续静音（帧数）；用来发现参数切换时插入的空白
fn longest_silence(samples: &[f32]) -> usize {
    let (mut longest, mut run) = (0, 0);
    for frame in samples.as_chunks::<2>().0 {
        if frame.iter().all(|s| s.abs() < 1e-4) {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    longest
}

#[test]
fn changing_rate_or_pitch_while_playing_inserts_no_gap() {
    // 拖动倍速/升降调滑块会连续切换多档；每次切换都不能在声音里插入静音（听起来是咔哒或停顿）
    let (mut engine, samples, _) = engine();
    engine.play();
    engine.pull(8_820);
    for (rate, pitch) in [(1.1, 0), (1.2, 0), (1.3, 1), (1.0, 2), (0.9, 0), (1.0, 0)] {
        engine.set_rate(rate);
        engine.set_pitch(pitch);
        engine.pull(4_410);
    }
    let output = samples.lock().unwrap();
    // 正弦波过零点只有一两个采样接近 0；超过 2ms 的静音就是插入的空白
    assert!(
        longest_silence(&output) < 88,
        "切换时插入了 {} 帧静音",
        longest_silence(&output)
    );
}
