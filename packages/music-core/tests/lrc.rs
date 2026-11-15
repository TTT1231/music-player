//! 验收：LRC 支持一行多个时间标签、两位或三位小数、`[offset]`；不是合法 UTF-8 时按 GBK 解码。

mod common;

use music_core::{LyricLine, decode_text, parse_lrc};

fn line(time_ms: u64, text: &str) -> LyricLine {
    LyricLine {
        time_ms,
        text: text.into(),
    }
}

#[test]
fn parses_fraction_with_one_two_or_three_digits() {
    let l = parse_lrc("[00:01]a\n[00:02.5]b\n[00:03.12]c\n[00:04.123]d\n[00:05:50]e");
    assert_eq!(
        l.lines,
        [
            line(1000, "a"),
            line(2500, "b"),
            line(3120, "c"),
            line(4123, "d"),
            line(5500, "e")
        ]
    );
}

#[test]
fn minutes_can_exceed_sixty() {
    assert_eq!(parse_lrc("[61:00.00]长曲").lines, [line(3_660_000, "长曲")]);
}

#[test]
fn multiple_tags_on_one_line_produce_one_line_each_sorted_by_time() {
    let l = parse_lrc("[00:01.00][00:05.00]副歌\n[00:03.00]主歌");
    assert_eq!(l.lines, [line(1000, "副歌"), line(3000, "主歌"), line(5000, "副歌")]);
}

#[test]
fn skips_metadata_and_empty_lines() {
    let l = parse_lrc("[ti:标题]\r\n[ar:歌手]\r\n[al:专辑]\r\n[by:制作]\r\n[00:01.00]\r\n\r\n[00:02.00]有字\r\n");
    assert_eq!(l.lines, [line(2000, "有字")]);
}

#[test]
fn trims_text() {
    assert_eq!(parse_lrc("[00:01.00]  前后有空格  ").lines, [line(1000, "前后有空格")]);
}

#[test]
fn positive_offset_moves_lines_earlier_and_clamps_at_zero() {
    let l = parse_lrc("[offset:500]\n[00:00.20]一\n[00:01.50]二");
    assert_eq!(l.lines, [line(0, "一"), line(1000, "二")]);
}

#[test]
fn negative_offset_moves_lines_later() {
    assert_eq!(parse_lrc("[offset:-250]\n[00:01.00]一").lines, [line(1250, "一")]);
}

#[test]
fn text_without_timed_lines_is_empty() {
    assert!(parse_lrc("").is_empty());
    assert!(parse_lrc("[ti:只有标题]\n普通文字").is_empty());
}

#[test]
fn decodes_utf8_and_strips_bom() {
    assert_eq!(decode_text("你好".as_bytes()), "你好");
    assert_eq!(decode_text("\u{feff}你好".as_bytes()), "你好");
}

#[test]
fn falls_back_to_gbk_when_not_utf8() {
    assert_eq!(decode_text(&[0xC4, 0xE3, 0xBA, 0xC3]), "你好");
}

#[test]
fn gbk_fixture_parses() {
    let bytes = std::fs::read(common::fixtures().join("Test Artist - Track Two.lrc")).unwrap();
    let lyrics = parse_lrc(&decode_text(&bytes));
    assert_eq!(lyrics.lines, [line(500, "旧编码的歌词"), line(2000, "第二句")]);
}
