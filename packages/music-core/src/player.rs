use std::ops::RangeInclusive;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::library::{FileRef, ImportReport, Playlist, TrackId};

/// 当前歌曲播放超过这个时长时，“上一首”回到本曲开头。
pub const RESTART_THRESHOLD: Duration = Duration::from_secs(3);
/// 倍速范围，步长 0.1。
pub const RATE_RANGE: RangeInclusive<f32> = 0.5..=1.5;
/// 升降调范围（半音）。
pub const PITCH_RANGE: RangeInclusive<i8> = -6..=6;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayMode {
    /// 顺序播放：最后一首播完后停止。
    Order,
    /// 列表循环。
    #[default]
    Loop,
    /// 随机播放：不连续选中同一首。
    Shuffle,
    /// 单曲循环。
    RepeatOne,
    /// 单曲播放：播完本曲后停止。
    Single,
}

/// 播放器要求外壳执行的动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// 加载并播放这首歌。
    Play(TrackId),
    /// 回到当前歌曲开头继续播放。
    Restart,
    /// 停止：回到当前歌曲开头并暂停。
    Stop,
}

/// 播放状态的唯一来源：播放列表、当前歌曲、播放与暂停、模式和各项调节。
/// 不接触音频输出，外壳根据返回的 [`Step`] 驱动音频引擎。
#[derive(Debug, Clone)]
pub struct Player {
    playlist: Playlist,
    current: Option<TrackId>,
    playing: bool,
    mode: PlayMode,
    volume: f32,
    rate: f32,
    pitch: i8,
    rng: u64,
}

impl Player {
    /// 新建播放器：未选中歌曲、暂停，列表循环，音量 0.8，倍速 1.0，不升降调。
    pub fn new(playlist: Playlist) -> Self {
        Self::with_seed(playlist, 0x9E37_79B9_7F4A_7C15)
    }

    /// 指定随机播放的种子。
    pub fn with_seed(playlist: Playlist, seed: u64) -> Self {
        Self {
            playlist,
            current: None,
            playing: false,
            mode: PlayMode::default(),
            volume: 0.8,
            rate: 1.0,
            pitch: 0,
            rng: seed,
        }
    }

    pub fn playlist(&self) -> &Playlist {
        &self.playlist
    }

    pub fn current(&self) -> Option<&TrackId> {
        self.current.as_ref()
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn mode(&self) -> PlayMode {
        self.mode
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn rate(&self) -> f32 {
        self.rate
    }

    pub fn pitch(&self) -> i8 {
        self.pitch
    }

    pub fn set_mode(&mut self, mode: PlayMode) {
        let _ = mode;
        todo!("Player::set_mode")
    }

    /// 音量限制在 0.0–1.0。
    pub fn set_volume(&mut self, volume: f32) {
        let _ = volume;
        todo!("Player::set_volume")
    }

    /// 倍速限制在 [`RATE_RANGE`] 并取整到 0.1，返回实际值。
    pub fn set_rate(&mut self, rate: f32) -> f32 {
        let _ = rate;
        todo!("Player::set_rate")
    }

    /// 升降调限制在 [`PITCH_RANGE`]，返回实际值。
    pub fn set_pitch(&mut self, semitones: i8) -> i8 {
        let _ = semitones;
        todo!("Player::set_pitch")
    }

    /// 选中并播放某首歌。
    pub fn play_track(&mut self, id: &TrackId) -> Step {
        let _ = id;
        todo!("Player::play_track")
    }

    pub fn play(&mut self) {
        todo!("Player::play")
    }

    pub fn pause(&mut self) {
        todo!("Player::pause")
    }

    /// 用户点“下一首”：任何模式都换歌；随机模式随机选另一首，其余按列表顺序并在末尾回到开头。
    pub fn next(&mut self) -> Option<Step> {
        todo!("Player::next")
    }

    /// 用户点“上一首”：已播放超过 [`RESTART_THRESHOLD`] 时回到本曲开头，否则换到上一首。
    pub fn prev(&mut self, position: Duration) -> Option<Step> {
        let _ = position;
        todo!("Player::prev")
    }

    /// 当前歌曲自然播完，按播放模式决定下一步。
    pub fn on_ended(&mut self) -> Step {
        todo!("Player::on_ended")
    }

    /// 当前歌曲无法读取（文件被移动或删除）：跳到列表中的下一首；列表只剩这一首时停止。
    pub fn on_unreadable(&mut self) -> Step {
        todo!("Player::on_unreadable")
    }

    pub fn import(&mut self, files: impl IntoIterator<Item = FileRef>) -> ImportReport {
        let _ = files.into_iter();
        todo!("Player::import")
    }

    /// 移出列表。移除的是当前歌曲时，选中原来排在它后面的歌（末尾则回到第一首），
    /// 播放状态不变并返回 `Some(Step::Play)`；列表变空时返回 `Some(Step::Stop)`。
    pub fn remove(&mut self, id: &TrackId) -> Option<Step> {
        let _ = id;
        todo!("Player::remove")
    }

    pub fn move_track(&mut self, from: usize, to: usize) {
        let _ = (from, to);
        todo!("Player::move_track")
    }

    #[allow(dead_code)]
    fn next_random(&mut self, below: usize) -> usize {
        let _ = (below, &mut self.rng);
        todo!("Player::next_random")
    }
}
