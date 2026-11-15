//! 拉取式播放引擎：解码和处理按需进行，播放位置只随实际输出前进。

use std::collections::VecDeque;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

use crate::{AudioError, RateConverter, StreamDecoder, Stretcher};

/// 输出设备（或测试输出）接受交错排列的 f32 采样。
pub trait AudioSink {
    fn sample_rate(&self) -> u32;
    fn channels(&self) -> u16;
    fn write(&mut self, samples: &[f32]);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    Ended,
    Error(String),
}

struct Chunk {
    samples: Vec<f32>,
    offset: usize,
    rate: f32,
}

/// 输出设备调用 `pull` 获取所需帧数；暂停和读完时可返回少于请求的帧数。
pub struct Engine<S: AudioSink> {
    sink: S,
    event: Box<dyn FnMut(EngineEvent) + Send>,
    decoder: Option<StreamDecoder>,
    stretcher: Option<Stretcher>,
    converter: Option<RateConverter>,
    pending: VecDeque<Chunk>,
    playing: bool,
    ended: bool,
    position_seconds: f64,
    duration: Duration,
    volume: f32,
    rate: f32,
    pitch: i8,
}

impl<S: AudioSink> Engine<S> {
    pub fn new(sink: S, event: impl FnMut(EngineEvent) + Send + 'static) -> Self {
        Self {
            sink,
            event: Box::new(event),
            decoder: None,
            stretcher: None,
            converter: None,
            pending: VecDeque::new(),
            playing: false,
            ended: false,
            position_seconds: 0.0,
            duration: Duration::ZERO,
            volume: 1.0,
            rate: 1.0,
            pitch: 0,
        }
    }

    pub fn load(&mut self, path: &Path) -> Result<Duration, AudioError> {
        self.load_decoder(StreamDecoder::open(path)?)
    }

    pub fn load_file(&mut self, file: File, ext_hint: Option<&str>) -> Result<Duration, AudioError> {
        self.load_decoder(StreamDecoder::from_file(file, ext_hint)?)
    }

    fn load_decoder(&mut self, decoder: StreamDecoder) -> Result<Duration, AudioError> {
        self.duration = decoder.duration();
        self.decoder = Some(decoder);
        self.position_seconds = 0.0;
        self.playing = false;
        self.reset_processing();
        Ok(self.duration)
    }

    fn reset_processing(&mut self) {
        self.pending.clear();
        self.ended = false;
        if let Some(decoder) = &self.decoder {
            let mut stretcher = Stretcher::new(decoder.sample_rate(), decoder.channels());
            stretcher.set_rate(self.rate);
            stretcher.set_pitch(self.pitch);
            self.stretcher = Some(stretcher);
            self.converter = Some(RateConverter::new(
                decoder.sample_rate(),
                self.sink.sample_rate(),
                self.sink.channels(),
            ));
        }
    }

    pub fn play(&mut self) {
        if self.decoder.is_some() && !self.ended {
            self.playing = true;
        }
    }
    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn seek(&mut self, position: Duration) -> Result<(), AudioError> {
        if let Some(decoder) = &mut self.decoder {
            let position = position.min(self.duration);
            decoder.seek(position)?;
            self.position_seconds = position.as_secs_f64();
            self.reset_processing();
        }
        Ok(())
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            1.0
        };
    }
    pub fn set_rate(&mut self, rate: f32) {
        self.flush_stretcher();
        self.rate = if rate.is_finite() { rate.clamp(0.25, 4.0) } else { 1.0 };
        if let Some(stretcher) = &mut self.stretcher {
            stretcher.set_rate(self.rate);
        }
    }
    pub fn set_pitch(&mut self, semitones: i8) {
        self.flush_stretcher();
        self.pitch = semitones.clamp(-24, 24);
        if let Some(stretcher) = &mut self.stretcher {
            stretcher.set_pitch(self.pitch);
        }
    }
    fn flush_stretcher(&mut self) {
        let (Some(decoder), Some(stretcher), Some(converter)) =
            (&self.decoder, &mut self.stretcher, &mut self.converter)
        else {
            return;
        };
        let mut stretched = Vec::new();
        stretcher.flush(&mut stretched);
        let mapped = map_channels(
            &stretched,
            usize::from(decoder.channels().max(1)),
            usize::from(self.sink.channels().max(1)),
        );
        let mut samples = Vec::new();
        converter.process(&mapped, &mut samples);
        if !samples.is_empty() {
            self.pending.push_back(Chunk {
                samples,
                offset: 0,
                rate: self.rate,
            });
        }
    }
    pub fn position(&self) -> Duration {
        Duration::from_secs_f64(self.position_seconds)
    }
    #[cfg(feature = "device")]
    pub(crate) fn sink_mut(&mut self) -> &mut S {
        &mut self.sink
    }
    #[cfg(feature = "device")]
    pub(crate) fn is_playing(&self) -> bool {
        self.playing
    }

    /// 拉取最多 `frames` 帧；返回实际写入设备的帧数。
    pub fn pull(&mut self, frames: usize) -> usize {
        if !self.playing {
            return 0;
        }
        let channels = usize::from(self.sink.channels().max(1));
        let mut written = 0;
        while written < frames {
            while self.pending.is_empty() && !self.ended {
                if let Err(error) = self.fill() {
                    self.playing = false;
                    (self.event)(EngineEvent::Error(error.to_string()));
                    break;
                }
            }
            let Some(chunk) = self.pending.front_mut() else {
                break;
            };
            let count = (frames - written).min((chunk.samples.len() - chunk.offset) / channels);
            let end = chunk.offset + count * channels;
            let mut samples = chunk.samples[chunk.offset..end].to_vec();
            for sample in &mut samples {
                *sample *= self.volume;
            }
            self.sink.write(&samples);
            chunk.offset = end;
            self.position_seconds = (self.position_seconds
                + count as f64 * f64::from(chunk.rate) / f64::from(self.sink.sample_rate()))
            .min(self.duration.as_secs_f64());
            written += count;
            if chunk.offset == chunk.samples.len() {
                self.pending.pop_front();
            }
        }
        if self.ended && self.pending.is_empty() && self.playing {
            self.playing = false;
            self.position_seconds = self.duration.as_secs_f64();
            (self.event)(EngineEvent::Ended);
        }
        written
    }

    fn fill(&mut self) -> Result<(), AudioError> {
        let Some(decoder) = &mut self.decoder else {
            return Ok(());
        };
        let mut decoded = Vec::new();
        let frames = decoder.read(&mut decoded)?;
        let input_channels = usize::from(decoder.channels().max(1));
        let output_channels = usize::from(self.sink.channels().max(1));
        let stretcher = self.stretcher.as_mut().expect("decoder has stretcher");
        let mut stretched = Vec::new();
        if frames == 0 {
            stretcher.flush(&mut stretched);
            self.ended = true;
        } else {
            stretcher.process(&decoded, &mut stretched);
        }
        let mapped = map_channels(&stretched, input_channels, output_channels);
        let converter = self.converter.as_mut().expect("decoder has converter");
        let mut output = Vec::new();
        converter.process(&mapped, &mut output);
        if self.ended {
            converter.flush(&mut output);
        }
        if !output.is_empty() {
            self.pending.push_back(Chunk {
                samples: output,
                offset: 0,
                rate: self.rate,
            });
        }
        Ok(())
    }
}

fn map_channels(input: &[f32], from: usize, to: usize) -> Vec<f32> {
    if from == to {
        return input.to_vec();
    }
    let mut output = Vec::with_capacity(input.len() / from * to);
    for frame in input.chunks_exact(from) {
        if to == 1 {
            output.push(frame.iter().sum::<f32>() / from as f32);
        } else {
            for channel in 0..to {
                output.push(frame[channel.min(from - 1)]);
            }
        }
    }
    output
}
