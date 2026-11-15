use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::library::{Track, TrackId};
use crate::player::{PlayMode, Player};

/// 重启后要恢复的内容：播放列表及顺序、上次的歌曲、播放模式和音量。倍速和升降调不保存。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub tracks: Vec<Track>,
    pub current: Option<TrackId>,
    pub mode: PlayMode,
    pub volume: f32,
}

impl Player {
    pub fn session(&self) -> Session {
        todo!("Player::session")
    }

    /// 从保存的内容恢复，处于暂停状态。
    pub fn restore(session: Session) -> Player {
        let _ = session;
        todo!("Player::restore")
    }
}

/// 写入 JSON：先写同目录临时文件再替换，中途退出不会留下半个文件。
pub fn save_session(path: &Path, session: &Session) -> io::Result<()> {
    let _ = (path, session);
    todo!("save_session")
}

/// 读取保存的内容；文件不存在时返回 `Ok(None)`。
pub fn load_session(path: &Path) -> io::Result<Option<Session>> {
    let _ = path;
    todo!("load_session")
}
