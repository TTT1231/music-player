mod common;

use std::fs::File;
use std::time::Duration;

use common::*;
use music_audio::{AudioError, StreamDecoder, decode_file};

#[test]
fn reads_mp3_and_ogg_one_block_at_a_time() {
    for name in [MP3_WITH_COVER, OGG_WITH_COVER, MP3_NO_COVER, OGG_NO_COVER] {
        let path = fixture(name);
        let expected = decode_file(&path).unwrap();
        let mut decoder = StreamDecoder::open(&path).unwrap();
        assert_eq!(decoder.sample_rate(), expected.sample_rate);
        assert_eq!(decoder.channels(), expected.channels);
        assert_close(decoder.duration().as_secs_f64(), TONE_SECS, 0.05, name);
        let mut buffer = Vec::new();
        let mut frames: usize = 0;
        loop {
            let read = decoder.read(&mut buffer).unwrap();
            assert_eq!(buffer.len(), read * expected.channels as usize);
            if read == 0 {
                break;
            }
            frames += read;
        }
        assert!(
            frames.abs_diff(expected.frames()) <= 1152,
            "{name}: {frames} vs {}",
            expected.frames()
        );
        assert_eq!(decoder.read(&mut buffer).unwrap(), 0);
    }
}

#[test]
fn seeks_to_requested_position_and_can_seek_back_after_end() {
    for name in [MP3_WITH_COVER, OGG_WITH_COVER] {
        let mut decoder = StreamDecoder::open(&fixture(name)).unwrap();
        decoder.seek(Duration::from_millis(1500)).unwrap();
        let mut buffer = Vec::new();
        let mut frames = 0;
        loop {
            let count = decoder.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            frames += count;
        }
        assert_close(frames as f64 / decoder.sample_rate() as f64, 1.5, 0.04, name);
        decoder.seek(Duration::ZERO).unwrap();
        assert!(decoder.read(&mut buffer).unwrap() > 0);
        let original = decode_file(&fixture(name)).unwrap();
        let error = buffer
            .iter()
            .zip(&original.samples)
            .map(|(a, b)| (a - b).abs() as f64)
            .sum::<f64>()
            / buffer.len() as f64;
        assert!(error < 0.02, "{name}: restart mean error {error}");
    }
}

#[test]
fn accepts_open_files_and_rejects_missing_or_corrupt_files() {
    let file = File::open(fixture(OGG_NO_COVER)).unwrap();
    assert!(StreamDecoder::from_file(file, Some("ogg")).is_ok());
    assert!(matches!(
        StreamDecoder::open(&fixture("不存在.mp3")),
        Err(AudioError::Io(_))
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.mp3");
    std::fs::write(&path, b"not audio".repeat(50)).unwrap();
    assert!(StreamDecoder::open(&path).is_err());
}

#[test]
fn seek_output_matches_the_decoded_position() {
    for name in [MP3_WITH_COVER, OGG_WITH_COVER] {
        let path = fixture(name);
        let all = decode_file(&path).unwrap();
        let mut decoder = StreamDecoder::open(&path).unwrap();
        for ms in [0, 100, 1500, 2800] {
            decoder.seek(Duration::from_millis(ms)).unwrap();
            let mut buffer = Vec::new();
            decoder.read(&mut buffer).unwrap();
            let offset = (all.sample_rate as usize * ms as usize / 1000) * all.channels as usize;
            let count = buffer.len().min(all.samples.len() - offset);
            assert!(count > 0, "{name}: no samples at {ms}ms");
            let mean_error = buffer[..count]
                .iter()
                .zip(&all.samples[offset..offset + count])
                .map(|(actual, expected)| (actual - expected).abs() as f64)
                .sum::<f64>()
                / count as f64;
            assert!(
                mean_error < 0.02,
                "{name} at {ms}ms: seek output mean error {mean_error}"
            );
        }
    }
}
