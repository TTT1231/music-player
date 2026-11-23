use std::collections::HashMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::name::parse_track_name;

/// 支持的音频扩展名（小写）。解码能力由 music-audio 保证。
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "ogg", "flac", "m4a", "wav", "aac"];

/// 待导入的文件：位置（桌面为路径，安卓为文件选择器返回的 URI）和显示用的文件名。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRef {
    pub location: String,
    pub file_name: String,
}

impl FileRef {
    pub fn new(location: impl Into<String>, file_name: impl Into<String>) -> Self {
        Self {
            location: location.into(),
            file_name: file_name.into(),
        }
    }

    pub fn from_path(path: &Path) -> Self {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self::new(path.to_string_lossy(), file_name)
    }
}

/// 歌曲标识，由配对键（去掉扩展名、忽略大小写和首尾空格的文件名）决定，重新导入同名歌曲时不变。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackId(pub String);

/// 曲库中的一首歌，保存音频和歌词位置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub title: String,
    pub artist: String,
    pub artists: Vec<String>,
    /// 音频文件位置。
    pub audio: String,
    /// 配对到的 `.lrc` 位置。
    pub lyrics: Option<String>,
    /// 时长（毫秒），未探测时为空；旧会话也按未探测处理。
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

/// 一次导入的结果，界面据此显示“已导入 N 首，M 首有歌词”。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// 新增或替换的歌曲，按导入顺序。
    pub added: Vec<TrackId>,
    /// `added` 中配对到歌词的数量。
    pub with_lyrics: usize,
    /// 只导入了歌词、补给列表中已有同名歌曲的数量。
    pub lyrics_attached: usize,
    /// 找不到同名歌曲、被丢弃的歌词数量。
    pub orphan_lyrics: usize,
}

/// 拆出不带扩展名的文件名和小写扩展名；没有扩展名时扩展名为空。
fn split_extension(file_name: &str) -> (&str, String) {
    match file_name.rsplit_once('.') {
        Some((base, ext)) if !base.is_empty() => (base, ext.to_ascii_lowercase()),
        _ => (file_name, String::new()),
    }
}

/// 配对键：去掉扩展名后的文件名，忽略大小写和首尾空格。
fn pair_key(base_name: &str) -> String {
    base_name.trim().to_lowercase()
}

/// 文件扩展名（不区分大小写）是否为支持的音频格式。
pub fn is_audio_file(file_name: &str) -> bool {
    let (_, ext) = split_extension(file_name);
    AUDIO_EXTENSIONS.contains(&ext.as_str())
}

/// 递归列出文件夹（含子文件夹）中的所有文件，按路径排序。
pub fn scan_dir(dir: &Path) -> io::Result<Vec<FileRef>> {
    let mut paths = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in std::fs::read_dir(&current)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                paths.push(entry.path());
            }
        }
    }
    paths.sort();
    Ok(paths.iter().map(|p| FileRef::from_path(p)).collect())
}

/// 曲库：全部歌曲及其文件，按导入顺序排列，就是界面上的「全部音乐」。
/// 播放顺序不在这里：播放列表和自建歌单都只是引用曲库中的歌曲。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Library {
    tracks: Vec<Track>,
}

impl Library {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_tracks(tracks: Vec<Track>) -> Self {
        Self { tracks }
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn get(&self, id: &TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| &t.id == id)
    }

    /// 更新已存在歌曲的时长；歌曲不存在时返回 false。
    pub fn set_duration(&mut self, id: &TrackId, duration_ms: u64) -> bool {
        let Some(track) = self.tracks.iter_mut().find(|track| &track.id == id) else {
            return false;
        };
        track.duration_ms = Some(duration_ms);
        true
    }

    pub fn position(&self, id: &TrackId) -> Option<usize> {
        self.tracks.iter().position(|t| &t.id == id)
    }

    /// 导入歌曲和歌词：按文件名配对；同名歌曲原位替换；只有歌词时补给已有的同名歌曲；
    /// 其他文件忽略。
    pub fn import(&mut self, files: impl IntoIterator<Item = FileRef>) -> ImportReport {
        let mut audio = Vec::new();
        let mut lyrics: HashMap<String, FileRef> = HashMap::new();
        for file in files {
            let (base, ext) = split_extension(&file.file_name);
            let key = pair_key(base);
            if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
                audio.push((key, base.to_string(), file));
            } else if ext == "lrc" {
                lyrics.entry(key).or_insert(file);
            }
        }

        let mut report = ImportReport::default();
        for (key, base, file) in audio {
            let id = TrackId(key.clone());
            let name = parse_track_name(&base);
            let previous_lyrics = self.get(&id).and_then(|t| t.lyrics.clone());
            let track = Track {
                id: id.clone(),
                title: name.title,
                artist: name.artist,
                artists: name.artists,
                audio: file.location,
                // 这次没带歌词时保留之前配对的歌词
                lyrics: lyrics.get(&key).map(|l| l.location.clone()).or(previous_lyrics),
                duration_ms: None,
            };
            match self.position(&id) {
                Some(pos) => self.tracks[pos] = track,
                None => self.tracks.push(track),
            }
            if !report.added.contains(&id) {
                report.added.push(id);
            }
        }

        let imported: Vec<&str> = report.added.iter().map(|id| id.0.as_str()).collect();
        for (key, file) in &lyrics {
            if imported.contains(&key.as_str()) {
                continue;
            }
            match self.tracks.iter_mut().find(|t| &t.id.0 == key) {
                Some(track) => {
                    track.lyrics = Some(file.location.clone());
                    report.lyrics_attached += 1;
                }
                None => report.orphan_lyrics += 1,
            }
        }

        report.with_lyrics = report
            .added
            .iter()
            .filter(|id| self.get(id).is_some_and(|t| t.lyrics.is_some()))
            .count();
        report
    }

    /// 移出曲库，不触碰磁盘上的文件。
    pub fn remove(&mut self, id: &TrackId) -> Option<Track> {
        let pos = self.position(id)?;
        Some(self.tracks.remove(pos))
    }
}
