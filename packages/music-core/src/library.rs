use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

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
        let _ = path;
        todo!("FileRef::from_path")
    }
}

/// 歌曲标识，由配对键（去掉扩展名、忽略大小写和首尾空格的文件名）决定，重新导入同名歌曲时不变。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackId(pub String);

/// 播放列表中的一首歌。只记录文件位置，不复制文件。
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

/// 文件扩展名（不区分大小写）是否为支持的音频格式。
pub fn is_audio_file(file_name: &str) -> bool {
    let _ = file_name;
    todo!("is_audio_file")
}

/// 递归列出文件夹（含子文件夹）中的所有文件，按路径排序。
pub fn scan_dir(dir: &Path) -> io::Result<Vec<FileRef>> {
    let _ = dir;
    todo!("scan_dir")
}

/// 有顺序的播放列表；上一首、下一首按这里的顺序走。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Playlist {
    tracks: Vec<Track>,
}

impl Playlist {
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

    pub fn position(&self, id: &TrackId) -> Option<usize> {
        self.tracks.iter().position(|t| &t.id == id)
    }

    /// 导入歌曲和歌词：按文件名配对；同名歌曲原位替换；只有歌词时补给已有的同名歌曲；
    /// 其他文件忽略。
    pub fn import(&mut self, files: impl IntoIterator<Item = FileRef>) -> ImportReport {
        let _ = files.into_iter();
        todo!("Playlist::import")
    }

    /// 移出列表，不触碰磁盘上的文件。
    pub fn remove(&mut self, id: &TrackId) -> Option<Track> {
        let _ = id;
        todo!("Playlist::remove")
    }

    /// 把 `from` 位置的歌曲移到 `to` 位置。
    pub fn move_track(&mut self, from: usize, to: usize) {
        let _ = (from, to);
        todo!("Playlist::move_track")
    }
}
