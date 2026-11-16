//! 系统音频设备输出：后台线程解码，设备回调只读取有界缓冲。

use std::collections::VecDeque;
use std::fs::File;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream};

use crate::{AudioError, AudioSink, Engine, EngineEvent};

const CHUNK_FRAMES: usize = 512;
/// 约 0.35 秒（48 kHz）。解码线程是普通优先级，窗口在后台、系统繁忙或安卓熄屏时可能被推迟调度；
/// 缓冲太小会被设备读空，空缺处补静音，听起来就是“滋滋”的断续声。
/// 暂停、跳转和音量都直接作用于缓冲，不受长度影响；只有倍速和升降调要等已缓冲部分播完才生效。
const BUFFER_FRAMES: usize = 16384;

struct DeviceSink {
    sample_rate: u32,
    channels: u16,
    samples: Vec<f32>,
}
impl AudioSink for DeviceSink {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn write(&mut self, samples: &[f32]) {
        self.samples.extend_from_slice(samples);
    }
}

struct Chunk {
    samples: Vec<f32>,
    offset: usize,
    start: f64,
    end: f64,
}
#[derive(Default)]
struct Buffer {
    chunks: VecDeque<Chunk>,
    frames: usize,
    position: f64,
    playing: bool,
    ended: bool,
    stop: bool,
    volume: f32,
    generation: u64,
}
type Shared = Arc<(Mutex<Buffer>, Condvar)>;
type Callback = Arc<Mutex<Box<dyn FnMut(EngineEvent) + Send>>>;
type EventGate = Arc<parking_lot::ReentrantMutex<()>>;

/// 系统设备流和后台处理线程。控制方法与输出回调共享一个有界缓冲。
pub struct DeviceOutput {
    _stream: Stream,
    engine: Arc<Mutex<Engine<DeviceSink>>>,
    shared: Shared,
    event_gate: EventGate,
    worker: Option<JoinHandle<()>>,
}

impl DeviceOutput {
    pub fn open(event: impl FnMut(EngineEvent) + Send + 'static) -> Result<Self, String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("没有可用的音频输出设备")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let shared: Shared = Arc::new((
            Mutex::new(Buffer {
                volume: 1.0,
                ..Buffer::default()
            }),
            Condvar::new(),
        ));
        let callback: Callback = Arc::new(Mutex::new(Box::new(event)));
        let event_gate = Arc::new(parking_lot::ReentrantMutex::new(()));
        let events = Arc::new(Mutex::new(VecDeque::new()));
        let engine_events = Arc::clone(&events);
        let sink = DeviceSink {
            sample_rate: config.sample_rate.0,
            channels: config.channels,
            samples: Vec::new(),
        };
        let engine = Arc::new(Mutex::new(Engine::new(sink, move |event| {
            engine_events.lock().unwrap_or_else(|e| e.into_inner()).push_back(event);
        })));
        let on_error = {
            let callback = Arc::clone(&callback);
            move |error: cpal::StreamError| {
                (callback.lock().unwrap_or_else(|e| e.into_inner()))(EngineEvent::Error(error.to_string()));
            }
        };
        let stream = match format {
            SampleFormat::F32 => build_stream::<f32>(&device, &config, Arc::clone(&shared), on_error),
            SampleFormat::I16 => build_stream::<i16>(&device, &config, Arc::clone(&shared), on_error),
            SampleFormat::U16 => build_stream::<u16>(&device, &config, Arc::clone(&shared), on_error),
            other => return Err(format!("不支持的音频输出格式：{other:?}")),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        let worker = {
            let engine = Arc::clone(&engine);
            let shared = Arc::clone(&shared);
            let event_gate = Arc::clone(&event_gate);
            let channels = usize::from(config.channels);
            thread::spawn(move || produce(engine, shared, events, callback, event_gate, channels))
        };
        Ok(Self {
            _stream: stream,
            engine,
            shared,
            event_gate,
            worker: Some(worker),
        })
    }

    pub fn load(&self, path: &Path) -> Result<Duration, AudioError> {
        let _gate = self.event_gate.lock();
        let mut engine = self.engine.lock().unwrap_or_else(|e| e.into_inner());
        let duration = engine.load(path)?;
        self.reset_buffer(0.0, false);
        Ok(duration)
    }
    pub fn load_file(&self, file: File, ext_hint: Option<&str>) -> Result<Duration, AudioError> {
        let _gate = self.event_gate.lock();
        let mut engine = self.engine.lock().unwrap_or_else(|e| e.into_inner());
        let duration = engine.load_file(file, ext_hint)?;
        self.reset_buffer(0.0, false);
        Ok(duration)
    }
    pub fn play(&self) {
        let _gate = self.event_gate.lock();
        let mut engine = self.engine.lock().unwrap_or_else(|e| e.into_inner());
        engine.play();
        resume_buffer(&self.shared, engine.is_playing());
    }
    pub fn pause(&self) {
        let _gate = self.event_gate.lock();
        self.engine.lock().unwrap_or_else(|e| e.into_inner()).pause();
        pause_buffer(&self.shared, &self.event_gate);
    }
    pub fn seek(&self, position: Duration) -> Result<(), AudioError> {
        let _gate = self.event_gate.lock();
        let mut engine = self.engine.lock().unwrap_or_else(|e| e.into_inner());
        engine.seek(position)?;
        let actual = engine.position().as_secs_f64();
        let playing = self.shared.0.lock().unwrap_or_else(|e| e.into_inner()).playing;
        if playing {
            engine.play();
        }
        self.reset_buffer(actual, playing);
        Ok(())
    }
    fn reset_buffer(&self, position: f64, playing: bool) {
        let (lock, wake) = &*self.shared;
        let mut buffer = lock.lock().unwrap_or_else(|e| e.into_inner());
        buffer.chunks.clear();
        buffer.frames = 0;
        buffer.position = position;
        buffer.playing = playing;
        buffer.ended = false;
        buffer.generation = buffer.generation.wrapping_add(1);
        wake.notify_one();
    }
    pub fn set_volume(&self, volume: f32) {
        self.shared.0.lock().unwrap_or_else(|e| e.into_inner()).volume = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            1.0
        };
    }
    pub fn set_rate(&self, rate: f32) {
        self.engine.lock().unwrap_or_else(|e| e.into_inner()).set_rate(rate);
    }
    pub fn set_pitch(&self, pitch: i8) {
        self.engine.lock().unwrap_or_else(|e| e.into_inner()).set_pitch(pitch);
    }
    pub fn position(&self) -> Duration {
        Duration::from_secs_f64(self.shared.0.lock().unwrap_or_else(|e| e.into_inner()).position)
    }
}

impl Drop for DeviceOutput {
    fn drop(&mut self) {
        let (lock, wake) = &*self.shared;
        lock.lock().unwrap_or_else(|e| e.into_inner()).stop = true;
        wake.notify_one();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn produce(
    engine: Arc<Mutex<Engine<DeviceSink>>>,
    shared: Shared,
    events: Arc<Mutex<VecDeque<EngineEvent>>>,
    callback: Callback,
    event_gate: EventGate,
    channels: usize,
) {
    loop {
        let (lock, wake) = &*shared;
        let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
        while !state.stop && (!state.playing || (state.frames >= BUFFER_FRAMES) || (state.ended && state.frames > 0)) {
            state = wake.wait(state).unwrap_or_else(|e| e.into_inner());
        }
        if state.stop {
            return;
        }
        if state.ended {
            let generation = state.generation;
            drop(state);
            dispatch_end(&shared, &event_gate, &callback, generation);
            continue;
        }
        drop(state);
        let mut engine = engine.lock().unwrap_or_else(|e| e.into_inner());
        let state = lock.lock().unwrap_or_else(|e| e.into_inner());
        if !state.playing || state.stop || state.ended {
            continue;
        }
        drop(state);
        let start = engine.position().as_secs_f64();
        let frames = engine.pull(CHUNK_FRAMES);
        let end = engine.position().as_secs_f64();
        let samples = std::mem::take(&mut engine.sink_mut().samples);
        let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
        if frames > 0 {
            debug_assert_eq!(samples.len(), frames * channels);
            state.frames += frames;
            state.chunks.push_back(Chunk {
                samples,
                offset: 0,
                start,
                end,
            });
        }
        let mut errors = Vec::new();
        let generation = state.generation;
        for event in events.lock().unwrap_or_else(|e| e.into_inner()).drain(..) {
            match event {
                EngineEvent::Ended => state.ended = true,
                error => {
                    state.playing = false;
                    errors.push(error);
                }
            }
        }
        if frames == 0 && !engine.is_playing() && !state.ended {
            state.playing = false;
        }
        drop(state);
        drop(engine);
        for error in errors {
            dispatch(&shared, &event_gate, &callback, generation, error);
        }
    }
}

fn pause_buffer(shared: &Shared, gate: &EventGate) {
    let _guard = gate.lock();
    let mut state = shared.0.lock().unwrap_or_else(|e| e.into_inner());
    state.playing = false;
    state.generation = state.generation.wrapping_add(1);
}

fn resume_buffer(shared: &Shared, engine_playing: bool) {
    let (lock, wake) = &**shared;
    let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
    state.playing = engine_playing || state.ended;
    wake.notify_one();
}

fn dispatch_end(shared: &Shared, gate: &EventGate, callback: &Callback, generation: u64) {
    let _guard = gate.lock();
    let mut state = shared.0.lock().unwrap_or_else(|e| e.into_inner());
    if state.generation != generation || !state.playing || !state.ended || state.frames > 0 {
        return;
    }
    state.playing = false;
    drop(state);
    (callback.lock().unwrap_or_else(|e| e.into_inner()))(EngineEvent::Ended);
}

fn dispatch(shared: &Shared, gate: &EventGate, callback: &Callback, generation: u64, event: EngineEvent) {
    let _guard = gate.lock();
    let current = shared.0.lock().unwrap_or_else(|e| e.into_inner()).generation;
    if current == generation {
        (callback.lock().unwrap_or_else(|e| e.into_inner()))(event);
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    shared: Shared,
    on_error: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, cpal::BuildStreamError>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    device.build_output_stream(
        config,
        move |output: &mut [T], _| {
            let (lock, wake) = &*shared;
            let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
            consume(&mut state, output, channels);
            wake.notify_one();
        },
        on_error,
        None,
    )
}

fn consume<T: cpal::Sample + FromSample<f32>>(state: &mut Buffer, output: &mut [T], channels: usize) {
    for frame in output.chunks_mut(channels) {
        if state.playing {
            let volume = state.volume;
            if let Some(chunk) = state.chunks.front_mut() {
                let count = frame.len().min(chunk.samples.len() - chunk.offset);
                for (dst, src) in frame.iter_mut().zip(&chunk.samples[chunk.offset..chunk.offset + count]) {
                    *dst = T::from_sample(*src * volume);
                }
                chunk.offset += count;
                let position =
                    chunk.start + (chunk.end - chunk.start) * chunk.offset as f64 / chunk.samples.len() as f64;
                let finished = chunk.offset == chunk.samples.len();
                state.position = position;
                state.frames = state.frames.saturating_sub(1);
                if finished {
                    state.chunks.pop_front();
                }
                continue;
            }
        }
        for sample in frame {
            *sample = T::from_sample(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_invalidates_pending_end_and_resume_reissues_end() {
        use std::sync::mpsc;
        let shared: Shared = Arc::new((
            Mutex::new(Buffer {
                playing: true,
                ended: true,
                ..Buffer::default()
            }),
            Condvar::new(),
        ));
        let gate = Arc::new(parking_lot::ReentrantMutex::new(()));
        let (tx, rx) = mpsc::channel();
        let callback: Callback = Arc::new(Mutex::new(Box::new(move |event| {
            tx.send(event).unwrap();
        })));
        let stale_generation = shared.0.lock().unwrap().generation;

        pause_buffer(&shared, &gate);
        resume_buffer(&shared, false);
        dispatch_end(&shared, &gate, &callback, stale_generation);
        assert!(rx.try_recv().is_err(), "暂停前排队的结束事件必须失效");
        assert!(shared.0.lock().unwrap().ended, "暂停和恢复后仍应保留已结束状态");

        let resumed_generation = shared.0.lock().unwrap().generation;
        dispatch_end(&shared, &gate, &callback, resumed_generation);
        assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), EngineEvent::Ended);
        assert!(!shared.0.lock().unwrap().playing);

        // 结束事件已送入上层队列、但上层先处理了暂停时，恢复仍须再次投递。
        pause_buffer(&shared, &gate);
        resume_buffer(&shared, false);
        let retried_generation = shared.0.lock().unwrap().generation;
        dispatch_end(&shared, &gate, &callback, retried_generation);
        assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), EngineEvent::Ended);
    }

    #[test]
    fn engine_does_not_play_before_a_file_is_loaded() {
        let sink = DeviceSink {
            sample_rate: 44100,
            channels: 2,
            samples: Vec::new(),
        };
        let mut engine = Engine::new(sink, |_| {});
        engine.play();
        assert!(!engine.is_playing());
    }

    #[test]
    fn seek_or_load_invalidates_pending_end_and_callback_can_reenter() {
        use std::sync::mpsc;
        let shared: Shared = Arc::new((Mutex::new(Buffer::default()), Condvar::new()));
        let gate = Arc::new(parking_lot::ReentrantMutex::new(()));
        let (tx, rx) = mpsc::channel();
        let callback: Callback = Arc::new(Mutex::new(Box::new(move |event| {
            tx.send(event).unwrap();
        })));
        let held = gate.lock();
        let (ready_tx, ready_rx) = mpsc::channel();
        let worker = {
            let shared = Arc::clone(&shared);
            let gate = Arc::clone(&gate);
            let callback = Arc::clone(&callback);
            thread::spawn(move || {
                ready_tx.send(()).unwrap();
                dispatch(&shared, &gate, &callback, 0, EngineEvent::Ended);
            })
        };
        ready_rx.recv().unwrap();
        shared.0.lock().unwrap().generation += 1;
        drop(held);
        worker.join().unwrap();
        assert!(rx.try_recv().is_err());

        let nested_gate = Arc::clone(&gate);
        let nested_state = Arc::clone(&shared);
        let nested: Callback = Arc::new(Mutex::new(Box::new(move |_| {
            let _guard = nested_gate.lock();
            nested_state.0.lock().unwrap().generation += 1;
        })));
        dispatch(&shared, &gate, &nested, 1, EngineEvent::Ended);
        assert_eq!(shared.0.lock().unwrap().generation, 2);
    }

    #[test]
    fn pause_preserves_buffer_and_position_and_volume_is_immediate() {
        let mut state = Buffer {
            playing: true,
            volume: 1.0,
            frames: 2,
            ..Buffer::default()
        };
        state.chunks.push_back(Chunk {
            samples: vec![1.0, 1.0, 0.5, 0.5],
            offset: 0,
            start: 1.0,
            end: 2.0,
        });
        let mut output = [0.0; 2];
        consume(&mut state, &mut output, 2);
        assert_eq!(output, [1.0, 1.0]);
        assert_eq!(state.position, 1.5);
        state.playing = false;
        consume(&mut state, &mut output, 2);
        assert_eq!(output, [0.0, 0.0]);
        assert_eq!(state.position, 1.5);
        assert_eq!(state.frames, 1);
        state.playing = true;
        state.volume = 0.5;
        consume(&mut state, &mut output, 2);
        assert_eq!(output, [0.25, 0.25]);
        assert_eq!(state.position, 2.0);
        assert_eq!(state.frames, 0);
    }

    #[test]
    fn end_event_waits_for_buffer_to_be_consumed() {
        use std::sync::mpsc;
        use std::time::Instant;

        let shared: Shared = Arc::new((
            Mutex::new(Buffer {
                volume: 1.0,
                playing: true,
                ..Buffer::default()
            }),
            Condvar::new(),
        ));
        let events = Arc::new(Mutex::new(VecDeque::new()));
        let engine_events = Arc::clone(&events);
        let sink = DeviceSink {
            sample_rate: 44100,
            channels: 2,
            samples: Vec::new(),
        };
        let mut engine = Engine::new(sink, move |event| engine_events.lock().unwrap().push_back(event));
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/music/subfolder/Test Artist - Track Three.ogg");
        engine.load(&path).unwrap();
        engine.play();
        let engine = Arc::new(Mutex::new(engine));
        let (tx, rx) = mpsc::channel();
        let callback: Callback = Arc::new(Mutex::new(Box::new(move |event| {
            tx.send(event).unwrap();
        })));
        let worker = {
            let shared = Arc::clone(&shared);
            let event_gate = Arc::new(parking_lot::ReentrantMutex::new(()));
            thread::spawn(move || produce(engine, shared, events, callback, event_gate, 2))
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(Instant::now() < deadline, "后台线程没有完成解码");
            let mut state = shared.0.lock().unwrap();
            if state.ended {
                assert!(state.frames > 0);
                assert!(rx.try_recv().is_err(), "缓冲尚未播放完就发送了结束事件");
                let frames = state.frames;
                let mut output = vec![0.0_f32; frames * 2];
                consume(&mut state, &mut output, 2);
                shared.1.notify_one();
                break;
            }
            let frames = state.frames;
            let mut output = vec![0.0_f32; frames * 2];
            consume(&mut state, &mut output, 2);
            shared.1.notify_one();
            drop(state);
            thread::yield_now();
        }
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), EngineEvent::Ended);
        shared.0.lock().unwrap().stop = true;
        shared.1.notify_one();
        worker.join().unwrap();
    }
}
