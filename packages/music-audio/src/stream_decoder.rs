use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Time, TimeBase, Timestamp};

use crate::AudioError;

/// 按音频包解码，输出交错排列的 f32 采样，不缓存整首歌曲。
pub struct StreamDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    sample_rate: u32,
    channels: u16,
    duration: Duration,
    pending: Option<Vec<f32>>,
    seek_target: Option<Timestamp>,
    ended: bool,
}

impl StreamDecoder {
    pub fn open(path: &Path) -> Result<Self, AudioError> {
        let file = File::open(path).map_err(AudioError::Io)?;
        Self::from_file(file, path.extension().and_then(|ext| ext.to_str()))
    }

    pub fn from_file(file: File, ext_hint: Option<&str>) -> Result<Self, AudioError> {
        let stream = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = ext_hint {
            hint.with_extension(ext);
        }
        let format = symphonia::default::get_probe()
            .probe(&hint, stream, FormatOptions::default(), MetadataOptions::default())
            .map_err(|_| AudioError::Unsupported)?;
        let track = format.default_track(TrackType::Audio).ok_or(AudioError::Unsupported)?;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .ok_or(AudioError::Unsupported)?;
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(|_| AudioError::Unsupported)?;
        let (track_id, time_base, frames, rate) = (track.id, track.time_base, track.num_frames, params.sample_rate);
        let mut result = Self {
            format,
            decoder,
            track_id,
            time_base,
            sample_rate: 0,
            channels: 0,
            duration: Duration::ZERO,
            pending: None,
            seek_target: None,
            ended: false,
        };
        let mut first = Vec::new();
        if result.read(&mut first)? == 0 {
            return Err(AudioError::Decode("没有可解码的音频".into()));
        }
        result.pending = Some(first);
        if let (Some(frames), Some(rate)) = (frames, rate.filter(|rate| *rate > 0)) {
            result.duration = Duration::from_secs_f64(frames as f64 / rate as f64);
        } else {
            // 缺少元数据时逐块统计，内存用量仍与歌曲长度无关。
            let mut buffer = Vec::new();
            let mut frames = 0usize;
            loop {
                let count = result.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                frames += count;
            }
            result.duration = Duration::from_secs_f64(frames as f64 / result.sample_rate as f64);
            result.seek(Duration::ZERO)?;
        }
        Ok(result)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn channels(&self) -> u16 {
        self.channels
    }
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// 替换 buffer 的内容，返回本块包含的帧数；0 表示结束。
    pub fn read(&mut self, buffer: &mut Vec<f32>) -> Result<usize, AudioError> {
        buffer.clear();
        if let Some(pending) = self.pending.take() {
            *buffer = pending;
            return Ok(buffer.len() / self.channels as usize);
        }
        if self.ended {
            return Ok(0);
        }
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => {
                    self.ended = true;
                    return Ok(0);
                }
                Err(SymphoniaError::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => {
                    self.ended = true;
                    return Ok(0);
                }
                Err(e) => return Err(AudioError::Decode(e.to_string())),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(decoded) => decoded,
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(e) => return Err(AudioError::Decode(e.to_string())),
            };
            let rate = decoded.spec().rate();
            let channels = decoded.spec().channels().count() as u16;
            if rate == 0 || channels == 0 {
                return Err(AudioError::Decode("无效的音频格式".into()));
            }
            if self.sample_rate != 0 && (self.sample_rate != rate || self.channels != channels) {
                return Err(AudioError::Decode("音频格式中途改变".into()));
            }
            self.sample_rate = rate;
            self.channels = channels;
            buffer.resize(decoded.samples_interleaved(), 0.0);
            decoded.copy_to_slice_interleaved(&mut *buffer);
            let frames = buffer.len() / channels as usize;
            if let (Some(target), Some(base)) = (self.seek_target, self.time_base) {
                let start = packet.pts.saturating_add(packet.trim_start);
                let ticks = target.get().saturating_sub(start.get()).max(0) as f64;
                let skip = (ticks * base.numer.get() as f64 / base.denom.get() as f64 * rate as f64).round() as usize;
                if frames == 0 || skip >= frames {
                    buffer.clear();
                    continue;
                }
                buffer.drain(..skip * channels as usize);
                self.seek_target = None;
            }
            if !buffer.is_empty() {
                return Ok(buffer.len() / channels as usize);
            }
        }
    }

    pub fn seek(&mut self, position: Duration) -> Result<(), AudioError> {
        if self.time_base.is_none() && !position.is_zero() {
            return Err(AudioError::Decode("缺少音轨时间基准，无法精确跳转".into()));
        }
        let time =
            Time::try_from_nanos_u128(position.as_nanos()).ok_or_else(|| AudioError::Decode("跳转位置过大".into()))?;
        let sought = self
            .format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time,
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| AudioError::Decode(e.to_string()))?;
        // Vorbis 等编码器需要先解码一个或多个包来恢复状态。
        // 从目标前开始预热，再按每个包的时间戳丢弃不需要的帧。
        if !position.is_zero() {
            let preroll = position.saturating_sub(Duration::from_millis(250));
            let preroll = Time::try_from_nanos_u128(preroll.as_nanos())
                .ok_or_else(|| AudioError::Decode("跳转位置过大".into()))?;
            self.format
                .seek(
                    SeekMode::Accurate,
                    SeekTo::Time {
                        time: preroll,
                        track_id: Some(self.track_id),
                    },
                )
                .map_err(|e| AudioError::Decode(e.to_string()))?;
        }
        self.decoder.reset();
        self.pending = None;
        self.ended = false;
        self.seek_target = self.time_base.map(|_| sought.required_ts);
        Ok(())
    }
}
