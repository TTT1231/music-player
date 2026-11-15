#![allow(dead_code)]

use std::path::PathBuf;

use music_audio::Pcm;

/// 仓库根的 fixtures/music/，由 scripts/gen-fixtures.mjs 生成：全部是 3 秒、44.1 kHz、440 Hz 正弦波。
pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/music")
        .join(name)
}

pub const MP3_WITH_COVER: &str = "Test Artist、Guest - Track One (Live).mp3";
pub const OGG_WITH_COVER: &str = "Test Artist - Track Two.ogg";
pub const MP3_NO_COVER: &str = "No Artist Instrumental.mp3";
pub const OGG_NO_COVER: &str = "subfolder/Test Artist - Track Three.ogg";

pub const TONE_HZ: f64 = 440.0;
pub const TONE_SECS: f64 = 3.0;

/// 用第一声道中间一半的过零次数估计正弦波频率，避开首尾的编码器填充和处理器启动段。
pub fn frequency(pcm: &Pcm) -> f64 {
    let ch = pcm.channels as usize;
    let mono: Vec<f32> = pcm.samples.iter().step_by(ch).copied().collect();
    let (from, to) = (mono.len() / 4, mono.len() * 3 / 4);
    let window = &mono[from..to];
    let crossings = window.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
    crossings as f64 / 2.0 / (window.len() as f64 / pcm.sample_rate as f64)
}

pub fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    let err = (actual - expected).abs() / expected;
    assert!(
        err <= tolerance,
        "{what}：实际 {actual:.3}，期望 {expected:.3}（允许 ±{:.0}%）",
        tolerance * 100.0
    );
}
