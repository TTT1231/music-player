use std::fs::File;
use std::path::Path;
use std::time::Duration;

use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, StandardVisualKey, Visual};

use crate::{AudioError, Cover, MediaInfo, Pcm};

/// 打开文件并探测格式；文件打不开是 `Io`，格式不认识是 `Unsupported`。
fn open(path: &Path) -> Result<Box<dyn FormatReader>, AudioError> {
    let file = File::open(path).map_err(AudioError::Io)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|_| AudioError::Unsupported)
}

/// 选中默认音频轨并创建解码器，返回轨道编号。
fn audio_decoder(format: &dyn FormatReader) -> Result<(u32, Box<dyn AudioDecoder>), AudioError> {
    let track = format.default_track(TrackType::Audio).ok_or(AudioError::Unsupported)?;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or(AudioError::Unsupported)?;
    let decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(|_| AudioError::Unsupported)?;
    Ok((track.id, decoder))
}

/// 把整个文件解码到内存。
pub fn decode_file(path: &Path) -> Result<Pcm, AudioError> {
    let mut format = open(path)?;
    let (track_id, mut decoder) = audio_decoder(format.as_ref())?;

    let mut pcm = Pcm {
        sample_rate: 0,
        channels: 0,
        samples: Vec::new(),
    };
    let mut chunk: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            // 结束、链式 ogg 的轨道切换、或文件尾部截断：到此为止
            Ok(None) | Err(SymphoniaError::ResetRequired) | Err(SymphoniaError::IoError(_)) => break,
            Err(e) if pcm.samples.is_empty() => return Err(AudioError::Decode(e.to_string())),
            Err(_) => break,
        };
        if packet.track_id != track_id {
            continue;
        }
        let buffer = match decoder.decode(&packet) {
            Ok(buffer) => buffer,
            // 单个坏包跳过，继续解码后面的
            Err(SymphoniaError::DecodeError(_)) | Err(SymphoniaError::IoError(_)) => continue,
            Err(e) => return Err(AudioError::Decode(e.to_string())),
        };
        if pcm.sample_rate == 0 {
            pcm.sample_rate = buffer.spec().rate();
            pcm.channels = buffer.spec().channels().count() as u16;
        }
        chunk.resize(buffer.samples_interleaved(), 0.0);
        buffer.copy_to_slice_interleaved(&mut chunk[..]);
        pcm.samples.extend_from_slice(&chunk);
    }

    if pcm.sample_rate == 0 {
        return Err(AudioError::Decode("没有可解码的音频".into()));
    }
    Ok(pcm)
}

/// 读取时长和内嵌封面（mp3 的 ID3 图片帧，ogg 的 METADATA_BLOCK_PICTURE）。
pub fn probe(path: &Path) -> Result<MediaInfo, AudioError> {
    let mut format = open(path)?;
    let cover = find_cover(format.as_mut());

    let track = format.default_track(TrackType::Audio).ok_or(AudioError::Unsupported)?;
    let sample_rate = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .and_then(|a| a.sample_rate);
    let duration = match (track.num_frames, sample_rate) {
        (Some(frames), Some(rate)) if rate > 0 => Duration::from_secs_f64(frames as f64 / rate as f64),
        // 容器没有记录总帧数时，解码一遍得到准确时长
        _ => decode_file(path)?.duration(),
    };
    Ok(MediaInfo { duration, cover })
}

/// 在全部元数据版本里找封面：优先正面封面，其次第一张图片。
fn find_cover(format: &mut dyn FormatReader) -> Option<Cover> {
    let mut visuals: Vec<Visual> = Vec::new();
    let mut metadata = format.metadata();
    loop {
        if let Some(revision) = metadata.current() {
            visuals.extend(revision.media.visuals.iter().cloned());
            for track in &revision.per_track {
                visuals.extend(track.metadata.visuals.iter().cloned());
            }
        }
        if metadata.is_latest() {
            break;
        }
        metadata.pop();
    }

    let chosen = visuals
        .iter()
        .find(|v| v.usage == Some(StandardVisualKey::FrontCover))
        .or_else(|| visuals.first())?;
    let mime = chosen
        .media_type
        .clone()
        .filter(|m| !m.is_empty())
        .or_else(|| sniff_image_type(&chosen.data).map(String::from))?;
    Some(Cover {
        mime,
        data: chosen.data.to_vec(),
    })
}

fn sniff_image_type(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else {
        None
    }
}
