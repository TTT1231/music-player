//! 验收：倍速 0.5–1.5 改变时长、不改变音高；升降调 -6 到 +6 个半音改变音高、不改变时长。

mod common;

use common::*;
use music_audio::{Pcm, Stretcher, decode_file, render};

fn tone() -> Pcm {
    decode_file(&fixture(OGG_NO_COVER)).unwrap()
}

#[test]
fn normal_settings_keep_duration_and_pitch() {
    let out = render(&tone(), 1.0, 0);
    assert_close(out.duration().as_secs_f64(), TONE_SECS, 0.03, "时长");
    assert_close(frequency(&out), TONE_HZ, 0.02, "频率");
}

#[test]
fn rate_changes_duration_but_not_pitch() {
    let input = tone();
    for rate in [0.5_f32, 0.8, 1.2, 1.5] {
        let out = render(&input, rate, 0);
        assert_eq!((out.sample_rate, out.channels), (input.sample_rate, input.channels));
        assert_close(
            out.duration().as_secs_f64(),
            TONE_SECS / rate as f64,
            0.03,
            &format!("{rate} 倍时长"),
        );
        assert_close(frequency(&out), TONE_HZ, 0.02, &format!("{rate} 倍频率"));
    }
}

#[test]
fn pitch_changes_frequency_but_not_duration() {
    let input = tone();
    for semitones in [-6_i8, -1, 1, 6] {
        let out = render(&input, 1.0, semitones);
        let expected = TONE_HZ * 2_f64.powf(semitones as f64 / 12.0);
        assert_close(
            out.duration().as_secs_f64(),
            TONE_SECS,
            0.03,
            &format!("{semitones} 半音时长"),
        );
        assert_close(frequency(&out), expected, 0.02, &format!("{semitones} 半音频率"));
    }
}

#[test]
fn rate_and_pitch_combine() {
    let out = render(&tone(), 1.5, 6);
    assert_close(out.duration().as_secs_f64(), TONE_SECS / 1.5, 0.03, "时长");
    assert_close(frequency(&out), TONE_HZ * 2_f64.sqrt(), 0.02, "频率");
}

#[test]
fn streaming_in_small_chunks_matches_offline_length() {
    // 实时播放按小块送入数据，结果长度应与一次性渲染一致
    let input = tone();
    let mut s = Stretcher::new(input.sample_rate, input.channels);
    s.set_rate(1.5);
    let mut out = Vec::new();
    for chunk in input.samples.chunks(512) {
        s.process(chunk, &mut out);
    }
    s.flush(&mut out);
    let offline = render(&input, 1.5, 0);
    let diff = (out.len() as f64 - offline.samples.len() as f64).abs();
    assert!(
        diff <= input.sample_rate as f64 * 0.02,
        "分块 {} 与离线 {} 采样数相差过大",
        out.len(),
        offline.samples.len()
    );
}
