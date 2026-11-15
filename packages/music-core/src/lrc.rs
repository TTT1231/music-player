/// 一行歌词：开始时间（毫秒，已应用 `[offset]`）和文字。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricLine {
    pub time_ms: u64,
    pub text: String,
}

/// 按时间排序的歌词；没有任何歌词行时界面显示“纯音乐，请欣赏”。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lyrics {
    pub lines: Vec<LyricLine>,
}

impl Lyrics {
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// 解析 LRC：一行可有多个时间标签，小数部分一到三位，支持 `[offset]`；跳过元数据和空行。
pub fn parse_lrc(text: &str) -> Lyrics {
    let _ = text;
    todo!("parse_lrc")
}

/// 歌词文件解码：去掉 UTF-8 BOM；不是合法 UTF-8 时按 GBK 解码。
pub fn decode_text(bytes: &[u8]) -> String {
    let _ = bytes;
    todo!("decode_text")
}
