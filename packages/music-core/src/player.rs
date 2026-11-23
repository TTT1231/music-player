use std::ops::RangeInclusive;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::library::{FileRef, ImportReport, Playlist, TrackId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPlaylist {
    pub id: String,
    pub name: String,
    pub track_ids: Vec<TrackId>,
}

/// 当前歌曲播放超过这个时长时，“上一首”回到本曲开头。
pub const RESTART_THRESHOLD: Duration = Duration::from_secs(3);
/// 倍速范围，步长 0.1。
pub const RATE_RANGE: RangeInclusive<f32> = 0.5..=1.5;
/// 升降调范围（半音）。
pub const PITCH_RANGE: RangeInclusive<i8> = -5..=5;

const DEFAULT_VOLUME: f32 = 0.8;
const DEFAULT_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

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
    pub(crate) playlist: Playlist,
    pub(crate) library: Playlist,
    pub(crate) saved_playlists: Vec<SavedPlaylist>,
    pub(crate) queue_source: Option<String>,
    pub(crate) current: Option<TrackId>,
    pub(crate) playing: bool,
    pub(crate) mode: PlayMode,
    pub(crate) volume: f32,
    rate: f32,
    pitch: i8,
    rng: u64,
}

impl Player {
    /// 新建播放器：未选中歌曲、暂停，列表循环，音量 0.8，倍速 1.0，不升降调。
    pub fn new(playlist: Playlist) -> Self {
        Self::with_seed(playlist, DEFAULT_SEED)
    }

    /// 指定随机播放的种子。
    pub fn with_seed(playlist: Playlist, seed: u64) -> Self {
        Self {
            library: playlist.clone(),
            playlist,
            saved_playlists: Vec::new(),
            queue_source: None,
            current: None,
            playing: false,
            mode: PlayMode::default(),
            volume: DEFAULT_VOLUME,
            rate: 1.0,
            pitch: 0,
            // xorshift 的状态不能为 0
            rng: if seed == 0 { DEFAULT_SEED } else { seed },
        }
    }

    pub fn playlist(&self) -> &Playlist {
        &self.playlist
    }

    pub fn library(&self) -> &Playlist {
        &self.library
    }
    pub fn saved_playlists(&self) -> &[SavedPlaylist] {
        &self.saved_playlists
    }
    pub fn queue_source(&self) -> Option<&str> {
        self.queue_source.as_deref()
    }

    pub fn create_saved_playlist(&mut self, name: &str) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 {
            return Err("歌单名称须为 1–40 个字符".into());
        }
        if name.to_lowercase() == "全部音乐"
            || self
                .saved_playlists
                .iter()
                .any(|p| p.name.to_lowercase() == name.to_lowercase())
        {
            return Err("这个名称已被使用".into());
        }
        let mut n = 1u64;
        while self.saved_playlists.iter().any(|p| p.id == format!("list-{n}")) {
            n += 1;
        }
        let id = format!("list-{n}");
        self.saved_playlists.push(SavedPlaylist {
            id: id.clone(),
            name: name.to_owned(),
            track_ids: Vec::new(),
        });
        Ok(id)
    }

    pub fn add_to_saved_playlist(&mut self, id: &str, tracks: &[TrackId]) -> Result<(), String> {
        let list = self
            .saved_playlists
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("歌单不存在")?;
        for track in tracks {
            if self.library.get(track).is_some() && !list.track_ids.contains(track) {
                list.track_ids.push(track.clone());
            }
        }
        Ok(())
    }

    pub fn remove_from_saved_playlist(&mut self, id: &str, track: &TrackId) -> Result<(), String> {
        let list = self
            .saved_playlists
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("歌单不存在")?;
        list.track_ids.retain(|entry| entry != track);
        Ok(())
    }

    pub fn play_saved_playlist(&mut self, id: Option<&str>, start: Option<&TrackId>) -> Result<Step, String> {
        let source = id_for_source(id);
        let tracks = match id {
            Some(id) => self
                .saved_playlists
                .iter()
                .find(|p| p.id == id)
                .ok_or("歌单不存在")?
                .track_ids
                .iter()
                .filter_map(|id| self.library.get(id).cloned())
                .collect(),
            None => self.library.tracks().to_vec(),
        };
        let playlist = Playlist::from_tracks(tracks);
        let id = start
            .cloned()
            .or_else(|| playlist.tracks().first().map(|track| track.id.clone()))
            .ok_or("歌单为空")?;
        if playlist.get(&id).is_none() {
            return Err("歌曲不在歌单中".into());
        }
        self.playlist = playlist;
        self.queue_source = Some(source);
        Ok(self.play_track(&id))
    }

    /// 更新歌曲的已探测时长；歌曲不存在时返回 false。
    pub fn set_duration(&mut self, id: &TrackId, duration_ms: u64) -> bool {
        let updated = self.library.set_duration(id, duration_ms);
        self.playlist.set_duration(id, duration_ms);
        updated
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
        self.mode = mode;
    }

    /// 音量限制在 0.0–1.0。
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = if volume.is_nan() { 0.0 } else { volume.clamp(0.0, 1.0) };
    }

    /// 倍速限制在 [`RATE_RANGE`] 并取整到 0.1，返回实际值。
    pub fn set_rate(&mut self, rate: f32) -> f32 {
        let rate = if rate.is_nan() { 1.0 } else { rate };
        let clamped = rate.clamp(*RATE_RANGE.start(), *RATE_RANGE.end());
        self.rate = (clamped * 10.0).round() / 10.0;
        self.rate
    }

    /// 升降调限制在 [`PITCH_RANGE`]，返回实际值。
    pub fn set_pitch(&mut self, semitones: i8) -> i8 {
        self.pitch = semitones.clamp(*PITCH_RANGE.start(), *PITCH_RANGE.end());
        self.pitch
    }

    /// 选中并播放某首歌；不在列表里时停止。
    pub fn play_track(&mut self, id: &TrackId) -> Step {
        if self.playlist.position(id).is_none() {
            return self.stop();
        }
        self.start(id.clone())
    }

    pub fn play(&mut self) {
        self.playing = self.current.is_some();
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// 用户点“下一首”：任何模式都换歌；随机模式随机选另一首，其余按列表顺序并在末尾回到开头。
    #[allow(clippy::should_implement_trait, reason = "“下一首”命令，不是迭代器")]
    pub fn next(&mut self) -> Option<Step> {
        let len = self.playlist.len();
        if len == 0 {
            return None;
        }
        let index = if self.mode == PlayMode::Shuffle {
            self.random_other()
        } else {
            self.current_index().map_or(0, |i| (i + 1) % len)
        };
        Some(self.start_at(index))
    }

    /// 用户点“上一首”：已播放超过 [`RESTART_THRESHOLD`] 时回到本曲开头，否则换到上一首。
    pub fn prev(&mut self, position: Duration) -> Option<Step> {
        let len = self.playlist.len();
        if len == 0 {
            return None;
        }
        if self.current.is_some() && position > RESTART_THRESHOLD {
            self.playing = true;
            return Some(Step::Restart);
        }
        let index = if self.mode == PlayMode::Shuffle {
            self.random_other()
        } else {
            self.current_index().map_or(len - 1, |i| (i + len - 1) % len)
        };
        Some(self.start_at(index))
    }

    /// 当前歌曲自然播完，按播放模式决定下一步。
    pub fn on_ended(&mut self) -> Step {
        let len = self.playlist.len();
        let Some(index) = self.current_index() else {
            return self.stop();
        };
        match self.mode {
            PlayMode::RepeatOne => {
                self.playing = true;
                Step::Restart
            }
            PlayMode::Single => self.stop(),
            PlayMode::Order if index + 1 == len => self.stop(),
            PlayMode::Shuffle => {
                let next = self.random_other();
                self.start_at(next)
            }
            PlayMode::Order | PlayMode::Loop => self.start_at((index + 1) % len),
        }
    }

    /// 当前歌曲无法读取（文件被移动或删除）：跳到列表中的下一首；列表只剩这一首时停止。
    pub fn on_unreadable(&mut self) -> Step {
        let len = self.playlist.len();
        match self.current_index() {
            Some(index) if len > 1 => self.start_at((index + 1) % len),
            _ => self.stop(),
        }
    }

    pub fn import(&mut self, files: impl IntoIterator<Item = FileRef>) -> ImportReport {
        self.import_inner(files, true)
    }

    pub fn import_library(&mut self, files: impl IntoIterator<Item = FileRef>) -> ImportReport {
        self.import_inner(files, false)
    }

    fn import_inner(&mut self, files: impl IntoIterator<Item = FileRef>, into_queue: bool) -> ImportReport {
        let report = self.library.import(files);
        let mut ids: Vec<_> = self.playlist.tracks().iter().map(|t| t.id.clone()).collect();
        if into_queue {
            for id in &report.added {
                if !ids.contains(id) {
                    ids.push(id.clone());
                }
            }
        }
        self.playlist = Playlist::from_tracks(ids.iter().filter_map(|id| self.library.get(id).cloned()).collect());
        report
    }

    /// 移出列表。移除的是当前歌曲时，选中原来排在它后面的歌（末尾则回到第一首），
    /// 播放状态不变并返回 `Some(Step::Play)`；列表变空时返回 `Some(Step::Stop)`。
    pub fn remove(&mut self, id: &TrackId) -> Option<Step> {
        self.library.remove(id);
        for list in &mut self.saved_playlists {
            list.track_ids.retain(|track| track != id);
        }
        self.remove_from_queue(id)
    }

    pub fn remove_from_queue(&mut self, id: &TrackId) -> Option<Step> {
        let removed_at = self.playlist.position(id)?;
        self.playlist.remove(id);
        if self.current.as_ref() != Some(id) {
            return None;
        }
        if self.playlist.is_empty() {
            self.current = None;
            self.playing = false;
            return Some(Step::Stop);
        }
        let next = self.playlist.tracks()[removed_at % self.playlist.len()].id.clone();
        self.current = Some(next.clone());
        Some(Step::Play(next))
    }

    pub fn move_track(&mut self, from: usize, to: usize) {
        self.playlist.move_track(from, to);
    }

    fn current_index(&self) -> Option<usize> {
        self.current.as_ref().and_then(|id| self.playlist.position(id))
    }

    fn start(&mut self, id: TrackId) -> Step {
        self.current = Some(id.clone());
        self.playing = true;
        Step::Play(id)
    }

    fn start_at(&mut self, index: usize) -> Step {
        let id = self.playlist.tracks()[index].id.clone();
        self.start(id)
    }

    fn stop(&mut self) -> Step {
        self.playing = false;
        Step::Stop
    }

    /// 随机选一首不是当前歌曲的位置；列表只有一首时就是它。
    fn random_other(&mut self) -> usize {
        let len = self.playlist.len();
        match self.current_index() {
            Some(current) if len > 1 => {
                let pick = self.next_random(len - 1);
                if pick >= current { pick + 1 } else { pick }
            }
            Some(current) => current,
            None => self.next_random(len),
        }
    }

    /// xorshift64*：结果在 `0..below`。
    fn next_random(&mut self, below: usize) -> usize {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) % below as u64) as usize
    }
}

fn id_for_source(id: Option<&str>) -> String {
    id.unwrap_or("all").to_owned()
}
