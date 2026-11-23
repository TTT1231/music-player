//! 解码、倍速与升降调、内嵌封面读取。
//!
//! 实时输出和离线渲染共用同一个 [`Stretcher`]，所以离线测试的结果代表实际听到的效果。

mod decode;
#[cfg(feature = "device")]
mod device;
mod engine;
mod rate_converter;
mod stream_decoder;
mod stretch;

use std::fmt;
use std::io;
use std::time::Duration;

pub use decode::{decode_file, probe, probe_file, sniff_image_mime};
#[cfg(feature = "device")]
pub use device::DeviceOutput;
pub use engine::{AudioSink, Engine, EngineEvent};
pub use rate_converter::RateConverter;
pub use stream_decoder::StreamDecoder;
pub use stretch::{Stretcher, render};

#[derive(Debug)]
pub enum AudioError {
    Io(io::Error),
    /// 不是支持的音频格式，或文件已损坏。
    Unsupported,
    Decode(String),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "读取失败：{e}"),
            Self::Unsupported => f.write_str("不支持的音频格式"),
            Self::Decode(m) => write!(f, "解码失败：{m}"),
        }
    }
}

impl std::error::Error for AudioError {}

/// 交错排列的 f32 采样。
#[derive(Debug, Clone, PartialEq)]
pub struct Pcm {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl Pcm {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(self.frames() as f64 / self.sample_rate.max(1) as f64)
    }
}

/// 内嵌封面图片。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    pub mime: String,
    pub data: Vec<u8>,
}

/// 不解码全部音频即可得到的信息。
#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    pub duration: Duration,
    pub cover: Option<Cover>,
}
