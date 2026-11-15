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
    let mut offset_ms: i64 = 0;
    let mut lines = Vec::new();

    for raw in text.lines() {
        let raw = raw.trim();
        if let Some((key, value)) = metadata_tag(raw) {
            if key.eq_ignore_ascii_case("offset") {
                offset_ms = value.trim().parse().unwrap_or(0);
            }
            continue;
        }

        let mut times = Vec::new();
        let mut rest = raw;
        while let Some(inner) = rest.strip_prefix('[') {
            let Some(end) = inner.find(']') else { break };
            let Some(ms) = parse_timestamp(&inner[..end]) else {
                break;
            };
            times.push(ms);
            rest = &inner[end + 1..];
        }

        let body = rest.trim();
        if body.is_empty() {
            continue;
        }
        lines.extend(times.into_iter().map(|time_ms| (time_ms, body)));
    }

    // 正的 offset 让歌词提前出现
    let mut lines: Vec<LyricLine> = lines
        .into_iter()
        .map(|(time_ms, text)| LyricLine {
            time_ms: (time_ms as i64 - offset_ms).max(0) as u64,
            text: text.to_string(),
        })
        .collect();
    lines.sort_by_key(|l| l.time_ms);
    Lyrics { lines }
}

/// 整行形如 `[ti:标题]`、`[offset:500]` 时返回标签名和值；标签名只含字母。
fn metadata_tag(line: &str) -> Option<(&str, &str)> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    let (key, value) = inner.split_once(':')?;
    (!key.is_empty() && key.chars().all(|c| c.is_ascii_alphabetic())).then_some((key, value))
}

/// `mm:ss`、`mm:ss.f`、`mm:ss.ff`、`mm:ss.fff`，小数分隔符也可以是冒号。
fn parse_timestamp(tag: &str) -> Option<u64> {
    let (minutes, rest) = tag.split_once(':')?;
    let (seconds, fraction) = match rest.find(['.', ':']) {
        Some(i) => (&rest[..i], Some(&rest[i + 1..])),
        None => (rest, None),
    };
    let minutes = digits(minutes, 3)?;
    let seconds = digits(seconds, 2)?;
    let fraction_ms = match fraction {
        None => 0,
        Some(f) => {
            let value = digits(f, 3)?;
            value * 10_u64.pow(3 - f.len() as u32)
        }
    };
    Some((minutes * 60 + seconds) * 1000 + fraction_ms)
}

fn digits(s: &str, max_len: usize) -> Option<u64> {
    if s.is_empty() || s.len() > max_len || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// 歌词文件解码：去掉 UTF-8 BOM；不是合法 UTF-8 时按 GBK 解码。
pub fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => encoding_rs::GBK.decode(bytes).0.into_owned(),
    }
}
