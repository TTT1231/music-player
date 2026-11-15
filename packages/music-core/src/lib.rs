//! 曲库、文件名解析、导入配对、LRC 解析、播放列表与播放模式、持久化。
//!
//! 不依赖 Tauri 和音频输出，可单独测试；职责划分见 docs/architecture.md。

mod library;
mod lrc;
mod name;
mod player;
mod session;

pub use library::{FileRef, ImportReport, Playlist, Track, TrackId, is_audio_file, scan_dir};
pub use lrc::{LyricLine, Lyrics, decode_text, parse_lrc};
pub use name::{TrackName, UNKNOWN_ARTIST, parse_track_name};
pub use player::{PITCH_RANGE, PlayMode, Player, RATE_RANGE, RESTART_THRESHOLD, Step};
pub use session::{Session, load_session, save_session};
