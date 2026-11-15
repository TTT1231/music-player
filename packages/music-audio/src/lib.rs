//! 解码、倍速与升降调、内嵌封面读取。
//!
//! 实时输出和离线渲染共用同一个 [`Stretcher`]，所以离线测试的结果代表实际听到的效果。

use std::fmt;
use std::io;
use std::path::Path;
use std::time::Duration;

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
        Duration::from_secs_f64(self.frames() as f64 / self.sample_rate as f64)
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

/// 把整个文件解码到内存。
pub fn decode_file(path: &Path) -> Result<Pcm, AudioError> {
    let _ = path;
    todo!("decode_file")
}

/// 读取时长和内嵌封面（mp3 的 ID3 图片帧，ogg 的 METADATA_BLOCK_PICTURE）。
pub fn probe(path: &Path) -> Result<MediaInfo, AudioError> {
    let _ = path;
    todo!("probe")
}

/// 时间伸缩处理器：倍速改变时长不改变音高，升降调改变音高不改变时长。
pub struct Stretcher {
    _private: (),
}

impl Stretcher {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        let _ = (sample_rate, channels);
        todo!("Stretcher::new")
    }

    pub fn set_rate(&mut self, rate: f32) {
        let _ = rate;
        todo!("Stretcher::set_rate")
    }

    pub fn set_pitch(&mut self, semitones: i8) {
        let _ = semitones;
        todo!("Stretcher::set_pitch")
    }

    /// 输入一段交错采样，把可用的输出追加到 `output`。
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        let _ = (input, output);
        todo!("Stretcher::process")
    }

    /// 输入结束，取出剩余输出。
    pub fn flush(&mut self, output: &mut Vec<f32>) {
        let _ = output;
        todo!("Stretcher::flush")
    }
}

/// 离线渲染：用 [`Stretcher`] 处理整段音频。
pub fn render(pcm: &Pcm, rate: f32, pitch_semitones: i8) -> Pcm {
    let _ = (pcm, rate, pitch_semitones);
    todo!("render")
}
