mod common;

use common::{TONE_HZ, TONE_SECS, assert_close, frequency};
use music_audio::{Pcm, RateConverter};

#[test]
fn converts_sample_rate_without_changing_duration_or_pitch() {
    let input: Vec<f32> = (0..132_300)
        .map(|i| (2.0 * std::f64::consts::PI * TONE_HZ * i as f64 / 44_100.0).sin() as f32)
        .collect();
    let mut converter = RateConverter::new(44_100, 48_000, 1);
    let mut samples = Vec::new();
    for chunk in input.chunks(511) {
        converter.process(chunk, &mut samples);
    }
    converter.flush(&mut samples);
    let output = Pcm {
        sample_rate: 48_000,
        channels: 1,
        samples,
    };
    assert_close(output.duration().as_secs_f64(), TONE_SECS, 0.001, "时长");
    assert_close(frequency(&output), TONE_HZ, 0.02, "频率");
}

#[test]
fn chunk_boundaries_do_not_change_stereo_output() {
    let input: Vec<f32> = (0..2_000).flat_map(|i| [i as f32, -(i as f32)]).collect();
    let convert = |chunk_size| {
        let mut converter = RateConverter::new(48_000, 44_100, 2);
        let mut output = Vec::new();
        for chunk in input.chunks(chunk_size) {
            converter.process(chunk, &mut output);
        }
        converter.flush(&mut output);
        output
    };
    assert_eq!(convert(2_000), convert(74));
}
