/// 文件名里没有“ - ”时使用的歌手名。
pub const UNKNOWN_ARTIST: &str = "未知歌手";

/// 从文件名解析出的歌手和歌名。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackName {
    /// 完整歌手字段，如“歌手甲、歌手乙”。
    pub artist: String,
    /// 按“、”拆开的各位歌手。
    pub artists: Vec<String>,
    pub title: String,
}

/// 解析去掉扩展名的文件名“歌手、歌手 - 歌名 (版本)”，只按第一个“ - ”拆分。
pub fn parse_track_name(base_name: &str) -> TrackName {
    let _ = base_name;
    todo!("parse_track_name")
}
