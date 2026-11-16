//! 验收：mp3 和 ogg（Vorbis）都能解码播放；内嵌封面能读出；无法读取的文件返回错误。

mod common;

use std::fs::File;

use common::*;
use music_audio::{AudioError, decode_file, probe, probe_file};

#[test]
fn decodes_mp3_and_ogg() {
    for name in [MP3_WITH_COVER, OGG_WITH_COVER, MP3_NO_COVER, OGG_NO_COVER] {
        let pcm = decode_file(&fixture(name)).unwrap_or_else(|e| panic!("{name}：{e}"));
        assert_eq!(pcm.sample_rate, 44_100, "{name}");
        assert_eq!(pcm.channels, 1, "{name}");
        assert_close(pcm.duration().as_secs_f64(), TONE_SECS, 0.05, &format!("{name} 时长"));
        assert_close(frequency(&pcm), TONE_HZ, 0.01, &format!("{name} 频率"));
    }
}

#[test]
fn probe_reports_duration() {
    for name in [MP3_WITH_COVER, OGG_WITH_COVER] {
        let info = probe(&fixture(name)).unwrap();
        assert_close(info.duration.as_secs_f64(), TONE_SECS, 0.05, &format!("{name} 时长"));
    }
}

#[test]
fn probe_file_matches_probe() {
    // 安卓从文件描述符打开歌曲：probe_file 与按路径 probe 的结果一致
    for name in [MP3_WITH_COVER, OGG_WITH_COVER, MP3_NO_COVER] {
        let path = fixture(name);
        let expected = probe(&path).unwrap();
        let actual = probe_file(
            File::open(&path).unwrap(),
            path.extension().and_then(|ext| ext.to_str()),
        )
        .unwrap();
        assert_close(
            actual.duration.as_secs_f64(),
            expected.duration.as_secs_f64(),
            0.01,
            name,
        );
        assert_eq!(actual.cover.is_some(), expected.cover.is_some(), "{name} 封面");
    }
}

#[test]
fn reads_embedded_png_cover_from_mp3_and_ogg() {
    for name in [MP3_WITH_COVER, OGG_WITH_COVER] {
        let cover = probe(&fixture(name))
            .unwrap()
            .cover
            .unwrap_or_else(|| panic!("{name} 应有封面"));
        assert_eq!(cover.mime, "image/png", "{name}");
        assert!(cover.data.starts_with(b"\x89PNG\r\n\x1a\n"), "{name} 封面数据应为 PNG");
    }
}

#[test]
fn files_without_cover_report_none() {
    for name in [MP3_NO_COVER, OGG_NO_COVER] {
        assert_eq!(probe(&fixture(name)).unwrap().cover, None, "{name}");
    }
}

#[test]
fn missing_file_is_an_io_error() {
    assert!(matches!(decode_file(&fixture("不存在.mp3")), Err(AudioError::Io(_))));
    assert!(matches!(probe(&fixture("不存在.mp3")), Err(AudioError::Io(_))));
}

#[test]
fn corrupt_file_is_an_error_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("坏文件.mp3");
    std::fs::write(&path, b"this is not audio at all, just some text bytes".repeat(50)).unwrap();
    assert!(decode_file(&path).is_err());
    assert!(probe(&path).is_err());
}
