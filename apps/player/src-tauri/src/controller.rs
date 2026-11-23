//! 播放状态控制器。音频设备和 Tauri 命令分别通过接口和事件接入。

use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

use music_core::{
    FileRef, ImportReport, Library, PlayMode, Player, PlaylistError, PlaylistId, Step, Track, TrackId, decode_text,
    load_session, parse_lrc, save_session,
};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Serialize;

use crate::library_files::{LibraryFiles, files_from_paths};

/** 把“位置”解析成可读文件：桌面是路径，安卓是 content:// URI（由插件打开文件描述符）。 */
pub type LocationResolver = Arc<dyn Fn(&str) -> io::Result<(File, Option<String>)> + Send + Sync>;

/** 桌面解析器：位置即路径。 */
pub fn path_resolver() -> LocationResolver {
    Arc::new(|location| {
        let path = PathBuf::from(location);
        let ext = path.extension().and_then(|ext| ext.to_str()).map(str::to_owned);
        Ok((File::open(&path)?, ext))
    })
}

pub trait AudioBackend {
    fn available(&self) -> bool {
        true
    }
    /// 输出设备丢失后重新打开当前默认设备；成功返回 true。
    fn recover(&mut self) -> bool {
        false
    }
    /// 释放暂停时无需占用的设备，保留最后的播放位置。
    fn release_idle(&mut self) -> bool {
        false
    }
    fn load(&mut self, location: &str) -> Result<Duration, String>;
    fn play(&mut self);
    fn pause(&mut self);
    fn seek(&mut self, position: Duration) -> Result<(), String>;
    fn set_volume(&mut self, volume: f32);
    fn set_rate(&mut self, rate: f32);
    fn set_pitch(&mut self, semitones: i8);
    fn position(&self) -> Duration;
}

impl AudioBackend for music_audio::DeviceOutput {
    fn load(&mut self, location: &str) -> Result<Duration, String> {
        // 直连实现按路径解析（桌面测试路径）
        let path = PathBuf::from(location);
        let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        let ext = path.extension().and_then(|ext| ext.to_str());
        self.load_file(file, ext).map_err(|e| e.to_string())
    }
    fn play(&mut self) {
        music_audio::DeviceOutput::play(self);
    }
    fn pause(&mut self) {
        music_audio::DeviceOutput::pause(self);
    }
    fn seek(&mut self, position: Duration) -> Result<(), String> {
        music_audio::DeviceOutput::seek(self, position).map_err(|e| e.to_string())
    }
    fn set_volume(&mut self, volume: f32) {
        music_audio::DeviceOutput::set_volume(self, volume);
    }
    fn set_rate(&mut self, rate: f32) {
        music_audio::DeviceOutput::set_rate(self, rate);
    }
    fn set_pitch(&mut self, semitones: i8) {
        music_audio::DeviceOutput::set_pitch(self, semitones);
    }
    fn position(&self) -> Duration {
        music_audio::DeviceOutput::position(self)
    }
}

/// 两端默认的导入行为及曲库快照的发布方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformBehavior {
    /// 默认是否把导入的歌追加到播放列表。桌面的曲库入口可以选择只入库。
    pub import_enqueues: bool,
    /// 是否单独发布曲库和歌单快照。桌面界面通过播放状态接收它们。
    pub projects_library: bool,
}

impl PlatformBehavior {
    pub const fn desktop() -> Self {
        Self {
            import_enqueues: true,
            projects_library: false,
        }
    }

    pub const fn android() -> Self {
        Self {
            import_enqueues: false,
            projects_library: true,
        }
    }
}

impl Default for PlatformBehavior {
    fn default() -> Self {
        if cfg!(target_os = "android") {
            Self::android()
        } else {
            Self::desktop()
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackView {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub has_lyrics: bool,
    pub duration_ms: Option<u64>,
    pub cover_url: String,
}
/// 播放状态：播放列表和当前播放。桌面队列面板与安卓播放页的「播放列表」弹层读它。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub tracks: Vec<TrackView>,
    pub library: Vec<TrackView>,
    pub playlists: Vec<PlaylistView>,
    pub queue_source: Option<String>,
    pub current_id: Option<String>,
    pub playing: bool,
    pub mode: PlayMode,
    pub volume: f32,
    pub rate: f32,
    pub pitch: i8,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistView {
    pub id: String,
    pub name: String,
    pub track_ids: Vec<String>,
}
/// 曲库和自建歌单：安卓的首页、歌单页和选歌页读它。`tracks` 即「全部音乐」，按导入顺序。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryState {
    pub tracks: Vec<TrackView>,
    pub playlists: Vec<PlaylistView>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub added: Vec<String>,
    pub with_lyrics: usize,
    pub lyrics_attached: usize,
    pub orphan_lyrics: usize,
    pub duplicates: usize,
    pub sources_kept: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControllerEvent {
    StateChanged,
    /// 曲库或歌单有变化：导入、移除、探测到时长、歌单的增删改名和增减歌曲。
    LibraryChanged,
    Notice {
        title: String,
    },
    DeviceError(String),
}

struct ProbeJob {
    id: TrackId,
    location: String,
    token: u64,
}
struct ProbeResult {
    job: ProbeJob,
    duration_ms: Option<u64>,
}

pub struct Controller<B: AudioBackend> {
    player: Player,
    backend: B,
    resolver: LocationResolver,
    behavior: PlatformBehavior,
    loaded: Option<TrackId>,
    session_path: PathBuf,
    dirty_since: Option<Instant>,
    events: Vec<ControllerEvent>,
    generation: Arc<AtomicU64>,
    probe_jobs: mpsc::Sender<ProbeJob>,
    probe_results: mpsc::Receiver<ProbeResult>,
    probe_tokens: HashMap<TrackId, u64>,
    next_probe_token: u64,
}

impl<B: AudioBackend> Controller<B> {
    pub fn new(
        session_path: PathBuf,
        mut backend: B,
        generation: Arc<AtomicU64>,
        resolver: LocationResolver,
    ) -> io::Result<Self> {
        let player = load_session(&session_path)?
            .map(Player::restore)
            .unwrap_or_else(|| Player::new(Library::new()));
        backend.set_volume(player.volume());
        let (jobs_tx, jobs_rx) = mpsc::channel::<ProbeJob>();
        let (results_tx, results_rx) = mpsc::channel();
        std::thread::spawn({
            let resolver = Arc::clone(&resolver);
            move || {
                while let Ok(job) = jobs_rx.recv() {
                    // 位置可能不是路径（安卓 URI），统一经解析器打开
                    let duration_ms = (resolver)(&job.location)
                        .and_then(|(file, ext)| music_audio::probe_file(file, ext.as_deref()).map_err(io::Error::other))
                        .ok()
                        .map(|info| millis(info.duration));
                    if results_tx.send(ProbeResult { job, duration_ms }).is_err() {
                        break;
                    }
                }
            }
        });
        let mut controller = Self {
            player,
            backend,
            resolver,
            behavior: PlatformBehavior::default(),
            loaded: None,
            session_path,
            dirty_since: None,
            events: Vec::new(),
            generation,
            probe_jobs: jobs_tx,
            probe_results: results_rx,
            probe_tokens: HashMap::new(),
            next_probe_token: 0,
        };
        let ids = controller
            .player
            .library()
            .tracks()
            .iter()
            .filter(|t| t.duration_ms.is_none())
            .map(|t| t.id.clone())
            .collect::<Vec<_>>();
        controller.schedule_probes(ids);
        Ok(controller)
    }

    /// 指定平台行为；默认跟随编译目标。
    pub fn set_behavior(&mut self, behavior: PlatformBehavior) {
        self.behavior = behavior;
    }

    pub fn state(&self) -> PlayerState {
        let current = self.player.current();
        let LibraryState {
            tracks: library,
            playlists,
        } = if self.behavior.projects_library {
            LibraryState::default()
        } else {
            self.library_state()
        };
        PlayerState {
            tracks: self
                .player
                .queue()
                .iter()
                .filter_map(|id| self.player.track(id))
                .map(track_view)
                .collect(),
            library,
            playlists,
            queue_source: self.player.queue_source().map(str::to_owned),
            current_id: current.map(|id| id.0.clone()),
            playing: self.player.is_playing(),
            mode: self.player.mode(),
            volume: self.player.volume(),
            rate: self.player.rate(),
            pitch: self.player.pitch(),
            position_ms: self.position_ms(),
            duration_ms: current
                .and_then(|id| self.player.track(id))
                .and_then(|track| track.duration_ms),
        }
    }
    fn position_ms(&self) -> u64 {
        let current = self.player.current();
        if current.is_some() && current == self.loaded.as_ref() {
            millis(self.backend.position())
        } else {
            0
        }
    }

    pub(crate) fn playing_position_ms(&self) -> Option<u64> {
        self.player.is_playing().then(|| self.position_ms())
    }

    /// 曲库和自建歌单的投影。
    pub fn library_state(&self) -> LibraryState {
        LibraryState {
            tracks: self.player.library().tracks().iter().map(track_view).collect(),
            playlists: self
                .player
                .playlists()
                .iter()
                .map(|playlist| PlaylistView {
                    id: playlist.id.0.clone(),
                    name: playlist.name.clone(),
                    track_ids: playlist.tracks.iter().map(|id| id.0.clone()).collect(),
                })
                .collect(),
        }
    }
    /// 读取封面等文件内容时使用；安卓位置是 content:// URI，不能当路径打开。
    pub fn resolver(&self) -> LocationResolver {
        Arc::clone(&self.resolver)
    }

    pub fn cover_path(&self, id: &str) -> Option<PathBuf> {
        self.player
            .track(&TrackId(id.to_owned()))
            .map(|track| PathBuf::from(&track.audio))
    }
    /// 导入桌面路径：文件夹递归展开，文件名取自路径。
    pub fn import_paths(&mut self, paths: &[String]) -> io::Result<ImportResult> {
        Ok(self.import_files(files_from_paths(paths)?))
    }

    /// 导入已知位置和文件名的文件（安卓：内容 URI 本身不含文件名，由系统文件选择器提供）。
    pub fn import_files(&mut self, files: Vec<FileRef>) -> ImportResult {
        let report = self.player.import(files);
        if self.behavior.import_enqueues {
            self.player.enqueue(report.added.iter().cloned());
        }
        self.finish_import(report, 0, 0)
    }

    /// 保存新曲库后才移走原文件。未被配对或被判作重复的来源留在原处。
    pub fn import_managed(
        &mut self,
        files: Vec<FileRef>,
        library: &LibraryFiles,
        remove_source: impl Fn(&str) -> io::Result<()>,
    ) -> io::Result<ImportResult> {
        self.import_managed_with_queue(files, library, remove_source, self.behavior.import_enqueues)
    }

    pub fn import_managed_with_queue(
        &mut self,
        files: Vec<FileRef>,
        library: &LibraryFiles,
        remove_source: impl Fn(&str) -> io::Result<()>,
        into_queue: bool,
    ) -> io::Result<ImportResult> {
        let (files, duplicates) = library.unique_files(&self.player, files);
        let mut preview = self.player.clone();
        let report = preview.import(files.iter().cloned());
        if report.added.is_empty() && report.lyrics_attached == 0 {
            return Ok(self.finish_import(report, duplicates, 0));
        }

        let (files, staged) = library.stage(&preview, files, &self.resolver)?;
        let mut next = self.player.clone();
        let report = next.import(files);
        if into_queue {
            next.enqueue(report.added.iter().cloned());
        }
        save_session(&self.session_path, &next.session())?;
        self.player = next;
        let sources_kept = staged.commit(remove_source);
        Ok(self.finish_import(report, duplicates, sources_kept))
    }

    fn finish_import(&mut self, report: ImportReport, duplicates: usize, sources_kept: usize) -> ImportResult {
        self.schedule_probes(report.added.clone());
        if let Some(id) = self.player.current().cloned()
            && self.loaded.as_ref() == Some(&id)
            && report.added.contains(&id)
        {
            let playing = self.player.is_playing();
            self.apply_step_with_playing(Step::Play(id), playing);
        }
        if !report.added.is_empty() || report.lyrics_attached > 0 {
            self.both_changed();
        }
        ImportResult {
            added: report.added.into_iter().map(|id| id.0).collect(),
            with_lyrics: report.with_lyrics,
            lyrics_attached: report.lyrics_attached,
            orphan_lyrics: report.orphan_lyrics,
            duplicates,
            sources_kept,
        }
    }
    /// 从某个歌单或搜索结果播放：播放列表换成 `ids` 的顺序，从 `start` 开始。
    /// 起点不在列表里时什么都不变，返回 false。
    pub fn play_from(&mut self, ids: &[String], start: &str) -> bool {
        let ids: Vec<TrackId> = ids.iter().cloned().map(TrackId).collect();
        let Some(step) = self.player.play_from(&ids, &TrackId(start.to_owned())) else {
            return false;
        };
        self.apply_step(step);
        self.changed();
        true
    }
    pub fn play_track(&mut self, id: &str) {
        let step = self.player.play_track(&TrackId(id.to_owned()));
        self.apply_step(step);
        self.changed();
    }
    /// 只移出播放列表：曲库、歌单和文件都不动。要连曲库里的文件一起删除，用 [`Self::remove_managed`]。
    pub fn remove_queue_track(&mut self, id: &str) {
        let id = TrackId(id.to_owned());
        if !self.player.queue().contains(&id) {
            return;
        }
        let step = self.player.remove(&id);
        self.finish_removal(&id, step);
        self.changed();
    }

    /// 从曲库移除歌曲并删除不再被引用的曲库文件；它同时离开播放列表和所有歌单。
    pub fn remove_managed(&mut self, id: &str, library: &LibraryFiles) -> io::Result<()> {
        let id = TrackId(id.into());
        let Some(track) = self.player.track(&id).cloned() else {
            return Ok(());
        };
        let mut next = self.player.clone();
        let step = next.remove_from_library(&id);
        save_session(&self.session_path, &next.session())?;
        self.player = next;
        self.finish_removal(&id, step);
        self.both_changed();
        // 同名图片与音频同生命周期：歌曲移出曲库时一并删除
        let sidecars = crate::cover::sidecar_paths(Path::new(&track.audio));
        library.remove_unreferenced(
            &self.player,
            [Some(track.audio), track.lyrics]
                .into_iter()
                .flatten()
                .chain(
                    library
                        .cover_path(&id.0)
                        .map(|path| path.to_string_lossy().into_owned()),
                )
                .chain(sidecars.into_iter().map(|path| path.to_string_lossy().into_owned())),
        )
    }

    fn finish_removal(&mut self, removed: &TrackId, step: Option<Step>) {
        let playing = self.player.is_playing();
        if let Some(step) = step {
            self.apply_step_with_playing(step, playing);
        }
        if self.loaded.as_ref() == Some(removed) {
            self.loaded = None;
        }
    }
    pub fn create_playlist(&mut self, name: &str) -> Result<String, PlaylistError> {
        let id = self.player.create_playlist(name)?;
        self.library_changed();
        Ok(id.0)
    }
    pub fn rename_playlist(&mut self, id: &str, name: &str) -> Result<(), PlaylistError> {
        self.player.rename_playlist(&PlaylistId(id.to_owned()), name)?;
        self.library_changed();
        Ok(())
    }
    /// 删除歌单；歌单里的歌曲仍留在曲库和播放列表里。
    pub fn delete_playlist(&mut self, id: &str) -> Result<(), PlaylistError> {
        self.player.delete_playlist(&PlaylistId(id.to_owned()))?;
        self.library_changed();
        Ok(())
    }
    /// 把歌曲按给定顺序加到歌单末尾，返回加入的数量（已在歌单里或曲库里没有的不算）。
    pub fn add_playlist_tracks(&mut self, id: &str, tracks: &[String]) -> Result<usize, PlaylistError> {
        let tracks: Vec<TrackId> = tracks.iter().cloned().map(TrackId).collect();
        let added = self.player.add_to_playlist(&PlaylistId(id.to_owned()), &tracks)?;
        self.library_changed();
        Ok(added)
    }
    /// 把歌曲移出歌单；曲库和播放列表不受影响。
    pub fn remove_playlist_track(&mut self, id: &str, track: &str) -> Result<(), PlaylistError> {
        self.player
            .remove_from_playlist(&PlaylistId(id.to_owned()), &TrackId(track.to_owned()))?;
        self.library_changed();
        Ok(())
    }
    pub fn play_playlist(&mut self, id: Option<&str>, track: Option<&str>) -> Result<(), PlaylistError> {
        let id = id.map(|id| PlaylistId(id.to_owned()));
        let track = track.map(|track| TrackId(track.to_owned()));
        let step = self.player.play_playlist(id.as_ref(), track.as_ref())?;
        self.apply_step(step);
        self.changed();
        Ok(())
    }

    pub fn play(&mut self) {
        if !self.backend.available() {
            if self.player.current().is_none() {
                let _ = self.player.next();
            }
            self.player.pause();
            self.backend.pause();
            self.generation.fetch_add(1, Ordering::SeqCst);
            self.changed();
            return;
        }
        if let Some(id) = self.player.current().cloned() {
            if self.loaded.as_ref() == Some(&id) {
                self.player.play();
                self.backend.play();
            } else {
                let step = self.player.play_track(&id);
                self.apply_step(step);
            }
        } else if let Some(step) = self.player.next() {
            self.apply_step(step);
        }
        self.changed();
    }
    pub fn pause(&mut self) {
        self.player.pause();
        self.backend.pause();
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.changed();
    }
    pub fn toggle(&mut self) {
        if self.player.is_playing() {
            self.pause()
        } else {
            self.play()
        }
    }
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) {
        if let Some(step) = self.player.next() {
            self.apply_step(step);
            self.changed();
        }
    }
    pub fn prev(&mut self) {
        if let Some(step) = self.player.prev(self.backend.position()) {
            self.apply_step(step);
            self.changed();
        }
    }
    pub fn seek(&mut self, ms: u64) -> Result<(), String> {
        if !self.backend.available() {
            return Err("没有可用的音频输出设备".into());
        }
        let id = self.player.current().cloned().ok_or("没有选中的歌曲")?;
        if self.loaded.as_ref() != Some(&id) {
            let playing = self.player.is_playing();
            self.apply_step_with_playing(Step::Play(id), playing);
            self.changed();
            if self.loaded.is_none() {
                return Err("无法加载歌曲".into());
            }
        }
        self.backend.seek(Duration::from_millis(ms))?;
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.events.push(ControllerEvent::StateChanged);
        Ok(())
    }
    pub fn set_mode(&mut self, mode: PlayMode) {
        self.player.set_mode(mode);
        self.changed();
    }
    pub fn set_volume(&mut self, volume: f32) {
        self.player.set_volume(volume);
        self.backend.set_volume(self.player.volume());
        self.changed();
    }
    pub fn set_rate(&mut self, rate: f32) -> f32 {
        let value = self.player.set_rate(rate);
        self.backend.set_rate(value);
        self.events.push(ControllerEvent::StateChanged);
        value
    }
    pub fn set_pitch(&mut self, semitones: i8) -> i8 {
        let value = self.player.set_pitch(semitones);
        self.backend.set_pitch(value);
        self.events.push(ControllerEvent::StateChanged);
        value
    }
    pub fn move_track(&mut self, from: usize, to: usize) {
        self.player.move_track(from, to);
        self.changed();
    }
    pub fn lyrics(&self, id: &str) -> io::Result<music_core::Lyrics> {
        let Some(path) = self
            .player
            .track(&TrackId(id.to_owned()))
            .and_then(|track| track.lyrics.as_ref())
        else {
            return Ok(parse_lrc(""));
        };
        let (mut file, _) = (self.resolver)(path)?;
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut bytes)?;
        Ok(parse_lrc(&decode_text(&bytes)))
    }
    pub fn on_ended(&mut self) {
        if !self.player.is_playing() {
            return;
        }
        let step = self.player.on_ended();
        self.apply_step(step);
        self.changed();
    }
    /// 设备丢失（拔耳机、蓝牙断开、睡眠唤醒、默认设备切换）后重新打开输出，
    /// 并把当前歌曲按原位置重新载入（保持暂停，由随后的播放命令继续）。
    pub fn recover_output(&mut self) {
        if self.backend.available() {
            return;
        }
        let resume = self.loaded.clone().map(|id| (id, self.backend.position()));
        if !self.backend.recover() {
            return;
        }
        self.loaded = None;
        self.backend.set_volume(self.player.volume());
        self.backend.set_rate(self.player.rate());
        self.backend.set_pitch(self.player.pitch());
        let Some((id, position)) = resume else {
            return;
        };
        let Some(location) = self.player.track(&id).map(|track| track.audio.clone()) else {
            return;
        };
        if self.backend.load(&location).is_ok() {
            self.loaded = Some(id);
            let _ = self.backend.seek(position);
            self.generation.fetch_add(1, Ordering::SeqCst);
        }
    }
    pub fn on_device_error(&mut self, error: String) {
        self.player.pause();
        self.backend.pause();
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.events.push(ControllerEvent::DeviceError(error));
        self.events.push(ControllerEvent::StateChanged);
    }
    pub fn save(&mut self) -> io::Result<()> {
        save_session(&self.session_path, &self.player.session())?;
        self.dirty_since = None;
        Ok(())
    }
    pub fn poll(&mut self) -> io::Result<Vec<ControllerEvent>> {
        if !self.player.is_playing() && self.backend.release_idle() {
            self.generation.fetch_add(1, Ordering::SeqCst);
        }
        while let Ok(result) = self.probe_results.try_recv() {
            let current = self.player.track(&result.job.id);
            if self.probe_tokens.get(&result.job.id) == Some(&result.job.token)
                && current.is_some_and(|track| track.audio == result.job.location && track.duration_ms.is_none())
                && let Some(duration) = result.duration_ms
                && self.player.set_duration(&result.job.id, duration)
            {
                // 时长只影响列表里的显示：播放列表里有这首歌才需要刷新播放状态
                if self.behavior.projects_library && self.player.queue().contains(&result.job.id) {
                    self.changed();
                }
                self.library_changed();
            }
        }
        if self
            .dirty_since
            .is_some_and(|time| time.elapsed() >= Duration::from_millis(500))
        {
            self.save()?;
        }
        Ok(std::mem::take(&mut self.events))
    }
    fn changed(&mut self) {
        self.dirty_since = Some(Instant::now());
        self.events.push(ControllerEvent::StateChanged);
    }
    fn library_changed(&mut self) {
        self.dirty_since = Some(Instant::now());
        let event = if self.behavior.projects_library {
            ControllerEvent::LibraryChanged
        } else {
            ControllerEvent::StateChanged
        };
        self.events.push(event);
    }
    /// 曲库和播放状态都有变化：导入、从曲库移除。
    fn both_changed(&mut self) {
        if self.behavior.projects_library {
            self.changed();
        }
        self.library_changed();
    }
    fn schedule_probes(&mut self, ids: Vec<TrackId>) {
        for id in ids {
            if let Some(track) = self.player.track(&id) {
                self.next_probe_token = self.next_probe_token.wrapping_add(1);
                self.probe_tokens.insert(id.clone(), self.next_probe_token);
                let _ = self.probe_jobs.send(ProbeJob {
                    id,
                    location: track.audio.clone(),
                    token: self.next_probe_token,
                });
            }
        }
    }
    fn apply_step(&mut self, step: Step) {
        self.apply_step_with_playing(step, true);
    }
    fn apply_step_with_playing(&mut self, mut step: Step, playing: bool) {
        let max_attempts = self.player.queue().len();
        let mut attempts = 0;
        loop {
            match step {
                Step::Play(ref id) => {
                    if !self.backend.available() {
                        self.player.pause();
                        self.backend.pause();
                        self.loaded = None;
                        break;
                    }
                    let Some(track) = self.player.track(id) else {
                        self.player.pause();
                        self.backend.pause();
                        break;
                    };
                    let location = track.audio.clone();
                    let title = track.title.clone();
                    let previous_duration = track.duration_ms;
                    self.backend.pause();
                    match self.backend.load(&location) {
                        Ok(duration) => {
                            self.generation.fetch_add(1, Ordering::SeqCst);
                            self.loaded = Some(id.clone());
                            let duration_ms = millis(duration);
                            if previous_duration != Some(duration_ms) {
                                self.player.set_duration(id, duration_ms);
                                if self.behavior.projects_library {
                                    self.library_changed();
                                }
                            }
                            if playing {
                                self.player.play();
                                self.backend.play();
                            } else {
                                self.player.pause();
                            }
                            break;
                        }
                        Err(_) => {
                            self.loaded = None;
                            self.events.push(ControllerEvent::Notice { title });
                            attempts += 1;
                            if attempts >= max_attempts {
                                self.player.pause();
                                self.backend.pause();
                                break;
                            }
                            step = self.player.on_unreadable();
                        }
                    }
                }
                Step::Restart => {
                    if self.seek(0).is_ok() {
                        self.player.play();
                        self.backend.play();
                    } else {
                        self.player.pause();
                        self.backend.pause();
                    }
                    break;
                }
                Step::Stop => {
                    self.player.pause();
                    self.backend.pause();
                    if self.loaded.is_some() {
                        let _ = self.seek(0);
                    }
                    break;
                }
            }
        }
    }
}
impl<B: AudioBackend> Drop for Controller<B> {
    fn drop(&mut self) {
        if self.dirty_since.is_some() {
            let _ = self.save();
        }
    }
}
fn millis(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn track_view(track: &Track) -> TrackView {
    TrackView {
        id: track.id.0.clone(),
        title: track.title.clone(),
        artist: track.artist.clone(),
        has_lyrics: track.lyrics.is_some(),
        duration_ms: track.duration_ms,
        cover_url: cover_url_for(&track.id.0, cfg!(any(target_os = "windows", target_os = "android"))),
    }
}

pub(crate) fn cover_url_for(id: &str, http_scheme: bool) -> String {
    let origin = if http_scheme {
        "http://cover.localhost/"
    } else {
        "cover://localhost/"
    };
    format!("{origin}{}", utf8_percent_encode(id, NON_ALPHANUMERIC))
}
