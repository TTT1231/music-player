use std::io;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::library::{Playlist, Track, TrackId};
use crate::player::{PlayMode, Player, SavedPlaylist};

/// 重启后要恢复的内容：播放列表及顺序、上次的歌曲、播放模式和音量。倍速和升降调不保存。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub library: Option<Vec<Track>>,
    #[serde(default)]
    pub saved_playlists: Vec<SavedPlaylist>,
    #[serde(default)]
    pub queue_source: Option<String>,
    pub current: Option<TrackId>,
    pub mode: PlayMode,
    pub volume: f32,
}

impl Player {
    pub fn session(&self) -> Session {
        Session {
            tracks: self.playlist.tracks().to_vec(),
            library: Some(self.library.tracks().to_vec()),
            saved_playlists: self.saved_playlists.clone(),
            queue_source: self.queue_source.clone(),
            current: self.current.clone(),
            mode: self.mode,
            volume: self.volume,
        }
    }

    /// 从保存的内容恢复，处于暂停状态。上次的歌曲已不在列表里时不选中任何歌曲。
    pub fn restore(session: Session) -> Player {
        let library = session.library.unwrap_or_else(|| session.tracks.clone());
        let mut player = Player::new(Playlist::from_tracks(library));
        player.playlist = Playlist::from_tracks(session.tracks);
        player.saved_playlists = session.saved_playlists;
        player.queue_source = session.queue_source;
        player.current = session.current.filter(|id| player.playlist.position(id).is_some());
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
