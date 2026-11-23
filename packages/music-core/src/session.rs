use std::io;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::library::{Library, Track, TrackId};
use crate::player::{PlayMode, Player};
use crate::playlist::Playlists;

/// 重启后要恢复的内容：曲库、播放列表及顺序、自建歌单、播放列表的来源、上次的歌曲、播放模式和音量。
/// 倍速和升降调不保存。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// 播放列表里的歌曲，按播放顺序。
    pub tracks: Vec<Track>,
    /// 曲库。最初的旧会话没有这一项，那时 `tracks` 既是曲库也是播放列表。
    #[serde(default)]
    pub library: Option<Vec<Track>>,
    /// 自建歌单。
    #[serde(default)]
    pub saved_playlists: Playlists,
    /// 播放列表最后一次是从哪里整份换来的：全部音乐是 [`crate::ALL_MUSIC_SOURCE`]，否则是歌单 id。
    #[serde(default)]
    pub queue_source: Option<String>,
    pub current: Option<TrackId>,
    pub mode: PlayMode,
    pub volume: f32,
    /// 以下两项只用来读入更早的安卓测试版写出的会话：那里 `tracks` 是曲库，播放列表按 id 存在 `queue`，
    /// 歌单存在 `playlists`。读到后换成上面的写法，不再写出。
    #[serde(default, skip_serializing)]
    queue: Option<Vec<TrackId>>,
    #[serde(default, skip_serializing)]
    playlists: Playlists,
}

impl Player {
    pub fn session(&self) -> Session {
        Session {
            tracks: self
                .queue
                .iter()
                .filter_map(|id| self.library.get(id).cloned())
                .collect(),
            library: Some(self.library.tracks().to_vec()),
            saved_playlists: self.playlists.clone(),
            queue_source: self.queue_source.clone(),
            current: self.current.clone(),
            mode: self.mode,
            volume: self.volume,
            queue: None,
            playlists: Playlists::default(),
        }
    }

    /// 从保存的内容恢复，处于暂停状态。歌单和播放列表里引用了曲库中已经没有的歌时忽略这些引用；
    /// 上次的歌曲已不在播放列表里时不选中任何歌曲。
    pub fn restore(session: Session) -> Player {
        // 三种会话：`tracks` 是曲库（更早的安卓测试版，带 queue）；`tracks` 是播放列表、曲库在 library
        // （带 library）；只有 `tracks`（最初的旧会话，它既是曲库也是播放列表）。
        let (library, ids, playlists) = match session.queue {
            Some(ids) => (session.tracks, ids, session.playlists),
            None => {
                let ids = session.tracks.iter().map(|track| track.id.clone()).collect();
                let library = session.library.unwrap_or(session.tracks);
                (library, ids, session.saved_playlists)
            }
        };
        let mut player = Player::new(Library::from_tracks(library));
        player.playlists = playlists;
        player
            .playlists
            .retain_tracks(|track| player.library.get(track).is_some());
        let mut queue = Vec::new();
        for id in ids {
            if player.library.get(&id).is_some() && !queue.contains(&id) {
                queue.push(id);
            }
        }
        player.queue = queue;
        player.queue_source = session.queue_source;
        player.current = session.current.filter(|id| player.queue.contains(id));
        player.set_mode(session.mode);
        player.set_volume(session.volume);
        player
    }
}

/// 写入 JSON：先写同目录临时文件再替换，中途退出不会留下半个文件。
pub fn save_session(path: &Path, session: &Session) -> io::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(session).map_err(io::Error::other)?;
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let mut file = std::fs::File::create(&temp)?;
    file.write_all(&json)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temp, path)
}

/// 读取保存的内容；文件不存在时返回 `Ok(None)`，内容损坏时返回 `InvalidData`。
pub fn load_session(path: &Path) -> io::Result<Option<Session>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
