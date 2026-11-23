//! 自建歌单：曲库中歌曲的有序列表。只保存歌曲 id，歌曲和文件都在曲库里。

use std::fmt;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::library::TrackId;

/// 内置的「全部音乐」就是曲库本身，不作为歌单保存；自建歌单不能叫这个名字。
pub const ALL_MUSIC_NAME: &str = "全部音乐";
/// 播放列表来自「全部音乐」时，`queue_source` 的值。
pub const ALL_MUSIC_SOURCE: &str = "all";
/// 歌单名称的最长字符数。
pub const MAX_PLAYLIST_NAME: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlaylistId(pub String);

/// 一个自建歌单：名称和按加入顺序排列的歌曲。
/// 会话文件里歌曲列表的字段名是 `trackIds`，`tracks` 是更早的安卓版本写出的名字，两种都能读。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: PlaylistId,
    pub name: String,
    #[serde(rename = "trackIds", alias = "tracks", default)]
    pub tracks: Vec<TrackId>,
}

/// 歌单操作被拒绝的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistError {
    NotFound,
    /// 歌单里没有歌，没法播放。
    Empty,
    /// 要从这首歌开始播，但它不在这个歌单里。
    NotInList,
    EmptyName,
    DuplicateName,
    ReservedName,
    NameTooLong,
}

impl fmt::Display for PlaylistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("歌单不存在"),
            Self::Empty => f.write_str("歌单为空"),
            Self::NotInList => f.write_str("歌曲不在歌单中"),
            Self::EmptyName => f.write_str("请输入歌单名称"),
            Self::DuplicateName => f.write_str("已经有同名的歌单了"),
            Self::ReservedName => f.write_str("「全部音乐」是内置歌单，请换一个名字"),
            Self::NameTooLong => write!(f, "歌单名称最多 {MAX_PLAYLIST_NAME} 个字"),
        }
    }
}

impl std::error::Error for PlaylistError {}

/// 全部自建歌单，按创建顺序排列。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Playlists(Vec<Playlist>);

impl From<Vec<Playlist>> for Playlists {
    fn from(playlists: Vec<Playlist>) -> Self {
        Self(playlists)
    }
}

impl Playlists {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Playlist> {
        self.0.iter()
    }

    pub fn get(&self, id: &PlaylistId) -> Option<&Playlist> {
        self.0.iter().find(|playlist| &playlist.id == id)
    }

    /// 新建空歌单。名称去掉首尾空白后不能为空，也不能与其他歌单或内置歌单重名（不区分大小写）。
    pub fn create(&mut self, name: &str, now: SystemTime) -> Result<PlaylistId, PlaylistError> {
        let name = checked_name(name)?;
        self.ensure_unique(&name, None)?;
        let id = self.fresh_id(now);
        self.0.push(Playlist {
            id: id.clone(),
            name,
            tracks: Vec::new(),
        });
        Ok(id)
    }

    /// 重命名，规则同 [`Self::create`]；保留原名总是允许的。
    pub fn rename(&mut self, id: &PlaylistId, name: &str) -> Result<(), PlaylistError> {
        let name = checked_name(name)?;
        self.ensure_unique(&name, Some(id))?;
        self.get_mut(id).ok_or(PlaylistError::NotFound)?.name = name;
        Ok(())
    }

    pub fn delete(&mut self, id: &PlaylistId) -> Result<(), PlaylistError> {
        let at = self.position(id).ok_or(PlaylistError::NotFound)?;
        self.0.remove(at);
        Ok(())
    }

    /// 把歌曲按给定顺序加到歌单末尾，已在歌单里的跳过。返回加入的数量。
    pub fn add_tracks(&mut self, id: &PlaylistId, tracks: &[TrackId]) -> Result<usize, PlaylistError> {
        let playlist = self.get_mut(id).ok_or(PlaylistError::NotFound)?;
        let mut added = 0;
        for track in tracks {
            if !playlist.tracks.contains(track) {
                playlist.tracks.push(track.clone());
                added += 1;
            }
        }
        Ok(added)
    }

    /// 把歌曲移出歌单；歌单里没有这首歌时什么都不做。
    pub fn remove_track(&mut self, id: &PlaylistId, track: &TrackId) -> Result<(), PlaylistError> {
        let playlist = self.get_mut(id).ok_or(PlaylistError::NotFound)?;
        playlist.tracks.retain(|item| item != track);
        Ok(())
    }

    /// 只保留 `keep` 认可的歌曲，用于歌曲离开曲库或会话引用了曲库里没有的歌。
    pub fn retain_tracks(&mut self, keep: impl Fn(&TrackId) -> bool) {
        for playlist in &mut self.0 {
            playlist.tracks.retain(|track| keep(track));
        }
    }

    fn position(&self, id: &PlaylistId) -> Option<usize> {
        self.0.iter().position(|playlist| &playlist.id == id)
    }

    fn get_mut(&mut self, id: &PlaylistId) -> Option<&mut Playlist> {
        self.0.iter_mut().find(|playlist| &playlist.id == id)
    }

    fn ensure_unique(&self, name: &str, except: Option<&PlaylistId>) -> Result<(), PlaylistError> {
        if name == ALL_MUSIC_NAME {
            return Err(PlaylistError::ReservedName);
        }
        let folded = name.to_lowercase();
        let taken = self
            .0
            .iter()
            .any(|playlist| playlist.name.to_lowercase() == folded && Some(&playlist.id) != except);
        if taken {
            return Err(PlaylistError::DuplicateName);
        }
        Ok(())
    }

    /// 以创建时刻的纳秒数作 id；同一时刻已经有歌单时加序号。
    fn fresh_id(&self, now: SystemTime) -> PlaylistId {
        let stamp = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let base = format!("p{stamp:x}");
        let mut candidate = base.clone();
        let mut serial = 0;
        while self.get(&PlaylistId(candidate.clone())).is_some() {
            serial += 1;
            candidate = format!("{base}_{serial}");
        }
        PlaylistId(candidate)
    }
}

/// 去掉首尾空白并检查长度。
fn checked_name(name: &str) -> Result<String, PlaylistError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(PlaylistError::EmptyName);
    }
    if name.chars().count() > MAX_PLAYLIST_NAME {
        return Err(PlaylistError::NameTooLong);
    }
    Ok(name.to_owned())
}
