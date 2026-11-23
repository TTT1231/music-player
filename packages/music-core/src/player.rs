use std::ops::RangeInclusive;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::library::{FileRef, ImportReport, Library, Track, TrackId};
use crate::playlist::{ALL_MUSIC_SOURCE, PlaylistError, PlaylistId, Playlists};

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

/// 播放状态的唯一来源：曲库、自建歌单、播放列表、当前歌曲、播放与暂停、模式和各项调节。
/// 三份列表互相独立：曲库保存歌曲和文件，歌单和播放列表只引用曲库中的歌曲。
/// 不接触音频输出，外壳根据返回的 [`Step`] 驱动音频引擎。
#[derive(Debug, Clone)]
pub struct Player {
    pub(crate) library: Library,
    /// 播放列表：上一首、下一首按这里的顺序走。
    pub(crate) queue: Vec<TrackId>,
    pub(crate) playlists: Playlists,
    /// 播放列表最后一次是从哪里整份换来的：[`ALL_MUSIC_SOURCE`] 或歌单 id。
    /// 之后调整播放列表（移出、排序、从搜索结果点播）不改变它。
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
    /// 新建播放器：未选中歌曲、暂停，列表循环，音量 0.8，倍速 1.0，不升降调；
    /// 播放列表先是整个曲库。
    pub fn new(library: Library) -> Self {
        Self::with_seed(library, DEFAULT_SEED)
    }

    /// 指定随机播放的种子。
    pub fn with_seed(library: Library, seed: u64) -> Self {
        let queue = library.tracks().iter().map(|track| track.id.clone()).collect();
        Self {
            library,
            queue,
            playlists: Playlists::default(),
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

    pub fn library(&self) -> &Library {
        &self.library
    }

    /// 播放列表里的歌曲，按播放顺序。
    pub fn queue(&self) -> &[TrackId] {
        &self.queue
    }

    pub fn playlists(&self) -> &Playlists {
        &self.playlists
    }

    /// 播放列表最后一次整份换来的来源；还没有从歌单点播过时是 `None`。
    pub fn queue_source(&self) -> Option<&str> {
        self.queue_source.as_deref()
    }

    /// 按歌曲 id 查曲库。
    pub fn track(&self, id: &TrackId) -> Option<&Track> {
        self.library.get(id)
    }

    /// 更新歌曲的已探测时长；歌曲不在曲库里时返回 false。
    pub fn set_duration(&mut self, id: &TrackId, duration_ms: u64) -> bool {
        self.library.set_duration(id, duration_ms)
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

    /// 选中并播放某首歌；不在播放列表里时停止。
    pub fn play_track(&mut self, id: &TrackId) -> Step {
        if !self.queue.contains(id) {
            return self.stop();
        }
        self.start(id.clone())
    }

    /// 从某个列表（如搜索结果）播放：播放列表换成 `ids` 的顺序（重复的和曲库里没有的忽略），
    /// 从 `start` 开始，播放列表不再对应任何歌单。`start` 不在结果里时不做任何事，返回 `None`。
    pub fn play_from(&mut self, ids: &[TrackId], start: &TrackId) -> Option<Step> {
        let mut queue = Vec::with_capacity(ids.len());
        for id in ids {
            if self.library.get(id).is_some() && !queue.contains(id) {
                queue.push(id.clone());
            }
        }
        if !queue.iter().any(|id| id == start) {
            return None;
        }
        self.queue = queue;
        self.queue_source = None;
        Some(self.start(start.clone()))
    }

    /// 从歌单播放：`None` 是「全部音乐」，否则是自建歌单。播放列表整份换成它的顺序，
    /// 从 `start` 开始（不指定则是第一首），并记下来源。
    pub fn play_playlist(&mut self, id: Option<&PlaylistId>, start: Option<&TrackId>) -> Result<Step, PlaylistError> {
        let tracks: Vec<TrackId> = match id {
            None => self.library.tracks().iter().map(|track| track.id.clone()).collect(),
            Some(id) => self.playlists.get(id).ok_or(PlaylistError::NotFound)?.tracks.clone(),
        };
        let start = start
            .cloned()
            .or_else(|| tracks.first().cloned())
            .ok_or(PlaylistError::Empty)?;
        let step = self.play_from(&tracks, &start).ok_or(PlaylistError::NotInList)?;
        self.queue_source = Some(id.map_or_else(|| ALL_MUSIC_SOURCE.to_owned(), |id| id.0.clone()));
        Ok(step)
    }

    /// 把歌曲追加到播放列表末尾，已在列表里或曲库里没有的跳过。返回加入的数量。
    pub fn enqueue(&mut self, ids: impl IntoIterator<Item = TrackId>) -> usize {
        let mut added = 0;
        for id in ids {
            if self.library.get(&id).is_some() && !self.queue.contains(&id) {
                self.queue.push(id);
                added += 1;
            }
        }
        added
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
        let len = self.queue.len();
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
        let len = self.queue.len();
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
        let len = self.queue.len();
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
        let len = self.queue.len();
        match self.current_index() {
            Some(index) if len > 1 => self.start_at((index + 1) % len),
            _ => self.stop(),
        }
    }

    /// 导入到曲库，按导入顺序排在末尾；不改变播放列表，需要入列时由外壳调用 [`Self::enqueue`]。
    pub fn import(&mut self, files: impl IntoIterator<Item = FileRef>) -> ImportReport {
        self.library.import(files)
    }

    /// 只移出播放列表，曲库、歌单和文件都不动。移除的是当前歌曲时，选中原来排在它后面的歌
    /// （末尾则回到第一首），播放状态不变并返回 `Some(Step::Play)`；列表变空时返回 `Some(Step::Stop)`。
    pub fn remove(&mut self, id: &TrackId) -> Option<Step> {
        let removed_at = self.queue.iter().position(|track| track == id)?;
        self.queue.remove(removed_at);
        if self.current.as_ref() != Some(id) {
            return None;
        }
        if self.queue.is_empty() {
            self.current = None;
            self.playing = false;
            return Some(Step::Stop);
        }
        let next = self.queue[removed_at % self.queue.len()].clone();
        self.current = Some(next.clone());
        Some(Step::Play(next))
    }

    /// 从曲库移除歌曲：同时移出播放列表和所有歌单。移除的是当前歌曲时按 [`Self::remove`] 的规则接下一首；
    /// 歌曲不在曲库里时返回 `None`。
    pub fn remove_from_library(&mut self, id: &TrackId) -> Option<Step> {
        self.library.remove(id)?;
        self.playlists.retain_tracks(|track| track != id);
        self.remove(id)
    }

    /// 把播放列表中 `from` 位置的歌曲移到 `to` 位置；越界时不做任何事。
    pub fn move_track(&mut self, from: usize, to: usize) {
        if from >= self.queue.len() || to >= self.queue.len() || from == to {
            return;
        }
        let id = self.queue.remove(from);
        self.queue.insert(to, id);
    }

    /// 新建空歌单，名称规则见 [`Playlists::create`]。
    pub fn create_playlist(&mut self, name: &str) -> Result<PlaylistId, PlaylistError> {
        self.playlists.create(name, SystemTime::now())
    }

    pub fn rename_playlist(&mut self, id: &PlaylistId, name: &str) -> Result<(), PlaylistError> {
        self.playlists.rename(id, name)
    }

    /// 删除歌单；歌单里的歌曲仍留在曲库，播放列表不受影响。
    pub fn delete_playlist(&mut self, id: &PlaylistId) -> Result<(), PlaylistError> {
        self.playlists.delete(id)
    }

    /// 把歌曲按给定顺序加到歌单末尾，已在歌单里或曲库里没有的跳过。返回加入的数量。
    pub fn add_to_playlist(&mut self, id: &PlaylistId, tracks: &[TrackId]) -> Result<usize, PlaylistError> {
        if self.playlists.get(id).is_none() {
            return Err(PlaylistError::NotFound);
        }
        let known: Vec<TrackId> = tracks
            .iter()
            .filter(|track| self.library.get(track).is_some())
            .cloned()
            .collect();
        self.playlists.add_tracks(id, &known)
    }

    /// 把歌曲移出歌单；曲库和播放列表不受影响。歌单里没有这首歌时什么都不做。
    pub fn remove_from_playlist(&mut self, id: &PlaylistId, track: &TrackId) -> Result<(), PlaylistError> {
        self.playlists.remove_track(id, track)
    }

    fn current_index(&self) -> Option<usize> {
        self.current
            .as_ref()
            .and_then(|id| self.queue.iter().position(|track| track == id))
    }

    fn start(&mut self, id: TrackId) -> Step {
        self.current = Some(id.clone());
        self.playing = true;
        Step::Play(id)
    }

    fn start_at(&mut self, index: usize) -> Step {
        let id = self.queue[index].clone();
        self.start(id)
    }

    fn stop(&mut self) -> Step {
        self.playing = false;
        Step::Stop
    }

    /// 随机选一个不是当前歌曲的位置；列表只有一首时就是它。
    fn random_other(&mut self) -> usize {
        let len = self.queue.len();
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
