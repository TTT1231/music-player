//! 验收：文件名“歌手、歌手 - 歌名 (版本)”解析出歌手和歌名；没有“ - ”时歌手为“未知歌手”。

use music_core::{UNKNOWN_ARTIST, parse_track_name};

#[test]
fn splits_artists_and_keeps_version_in_title() {
    let n = parse_track_name("测试歌手、合唱者 - 第一首 (现场版)");
    assert_eq!(n.artist, "测试歌手、合唱者");
    assert_eq!(n.artists, ["测试歌手", "合唱者"]);
    assert_eq!(n.title, "第一首 (现场版)");
}

#[test]
fn without_separator_uses_unknown_artist_and_whole_name() {
    let n = parse_track_name("没有歌手的纯音乐");
    assert_eq!(n.artist, UNKNOWN_ARTIST);
    assert_eq!(n.artists, [UNKNOWN_ARTIST]);
    assert_eq!(n.title, "没有歌手的纯音乐");
}

#[test]
fn splits_only_on_first_separator() {
    let n = parse_track_name("歌手 - 歌名 - 副标题");
    assert_eq!(n.artist, "歌手");
    assert_eq!(n.title, "歌名 - 副标题");
}

#[test]
fn hyphen_without_spaces_is_part_of_the_title() {
    let n = parse_track_name("A-ha");
    assert_eq!(n.artist, UNKNOWN_ARTIST);
    assert_eq!(n.title, "A-ha");
}

#[test]
fn trims_whitespace_around_parts() {
    let n = parse_track_name("  歌手  -  歌名  ");
    assert_eq!(n.artist, "歌手");
    assert_eq!(n.title, "歌名");
}
