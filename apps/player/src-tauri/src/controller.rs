//! 播放状态控制器。音频设备和 Tauri 命令分别通过接口和事件接入。

use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

use music_core::{
    FileRef, PlayMode, Player, Playlist, Step, TrackId, decode_text, load_session, parse_lrc, save_session, scan_dir,
};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Serialize;

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
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub tracks: Vec<TrackView>,
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
pub struct ImportResult {
    pub added: Vec<String>,
    pub with_lyrics: usize,
    pub lyrics_attached: usize,
    pub orphan_lyrics: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControllerEvent {
    StateChanged,
    Notice { title: String },
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
            .unwrap_or_else(|| Player::new(Playlist::new()));
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
            .playlist()
            .tracks()
            .iter()
            .filter(|t| t.duration_ms.is_none())
            .map(|t| t.id.clone())
            .collect::<Vec<_>>();
        controller.schedule_probes(ids);
        Ok(controller)
    }

    pub fn state(&self) -> PlayerState {
        let tracks = self
            .player
            .playlist()
            .tracks()
            .iter()
            .map(|track| TrackView {
                id: track.id.0.clone(),
                title: track.title.clone(),
                artist: track.artist.clone(),
                has_lyrics: track.lyrics.is_some(),
                duration_ms: track.duration_ms,
                cover_url: cover_url_for(&track.id.0, cfg!(any(target_os = "windows", target_os = "android"))),
            })
            .collect();
        let current = self.player.current();
        PlayerState {
            tracks,
            current_id: current.map(|id| id.0.clone()),
            playing: self.player.is_playing(),
            mode: self.player.mode(),
            volume: self.player.volume(),
            rate: self.player.rate(),
            pitch: self.player.pitch(),
            position_ms: if current.is_some() && current == self.loaded.as_ref() {
                millis(self.backend.position())
            } else {
                0
            },
            duration_ms: current
                .and_then(|id| self.player.playlist().get(id))
                .and_then(|track| track.duration_ms),
        }
    }
    /// 读取封面等文件内容时使用；安卓位置是 content:// URI，不能当路径打开。
    pub fn resolver(&self) -> LocationResolver {
        Arc::clone(&self.resolver)
    }

    pub fn cover_path(&self, id: &str) -> Option<PathBuf> {
        self.player
            .playlist()
            .get(&TrackId(id.to_owned()))
            .map(|track| PathBuf::from(&track.audio))
    }
    /// 导入桌面路径：文件夹递归展开，文件名取自路径。
    pub fn import_paths(&mut self, paths: &[String]) -> io::Result<ImportResult> {
        let mut files = Vec::new();
        for value in paths {
            let path = Path::new(value);
            if path.is_dir() {
                files.extend(scan_dir(path)?);
            } else {
                files.push(FileRef::from_path(path));
            }
        }
        Ok(self.import_files(files))
    }

    /// 导入已知位置和文件名的文件（安卓：内容 URI 本身不含文件名，由系统文件选择器提供）。
    pub fn import_files(&mut self, files: Vec<FileRef>) -> ImportResult {
        let report = self.player.import(files);
        self.schedule_probes(report.added.clone());
        if let Some(id) = self.player.current().cloned()
            && self.loaded.as_ref() == Some(&id)
            && report.added.contains(&id)
        {
            let playing = self.player.is_playing();
            self.apply_step_with_playing(Step::Play(id), playing);
        }
        if !report.added.is_empty() || report.lyrics_attached > 0 {
            self.changed();
        }
        ImportResult {
            added: report.added.into_iter().map(|id| id.0).collect(),
            with_lyrics: report.with_lyrics,
            lyrics_attached: report.lyrics_attached,
            orphan_lyrics: report.orphan_lyrics,
        }
    }
    pub fn play_track(&mut self, id: &str) {
        let step = self.player.play_track(&TrackId(id.to_owned()));
        self.apply_step(step);
        self.changed();
    }
    pub fn remove_track(&mut self, id: &str) {
        let id = TrackId(id.to_owned());
        if self.player.playlist().get(&id).is_none() {
            return;
        }
        let playing = self.player.is_playing();
        if let Some(step) = self.player.remove(&id) {
            self.apply_step_with_playing(step, playing);
        }
        self.changed();
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
            .playlist()
            .get(&TrackId(id.to_owned()))
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
        let Some(location) = self.player.playlist().get(&id).map(|track| track.audio.clone()) else {
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
        while let Ok(result) = self.probe_results.try_recv() {
            let current = self.player.playlist().get(&result.job.id);
            if self.probe_tokens.get(&result.job.id) == Some(&result.job.token)
                && current.is_some_and(|track| track.audio == result.job.location && track.duration_ms.is_none())
                && let Some(duration) = result.duration_ms
                && self.player.set_duration(&result.job.id, duration)
            {
                self.changed();
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
    fn schedule_probes(&mut self, ids: Vec<TrackId>) {
        for id in ids {
            if let Some(track) = self.player.playlist().get(&id) {
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
        let max_attempts = self.player.playlist().len();
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
                    let Some(track) = self.player.playlist().get(id) else {
                        self.player.pause();
                        self.backend.pause();
                        break;
                    };
                    let location = track.audio.clone();
                    let title = track.title.clone();
                    self.backend.pause();
                    match self.backend.load(&location) {
                        Ok(duration) => {
                            self.generation.fetch_add(1, Ordering::SeqCst);
                            self.loaded = Some(id.clone());
                            self.player.set_duration(id, millis(duration));
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

pub(crate) fn cover_url_for(id: &str, http_scheme: bool) -> String {
    let origin = if http_scheme {
        "http://cover.localhost/"
    } else {
        "cover://localhost/"
    };
    format!("{origin}{}", utf8_percent_encode(id, NON_ALPHANUMERIC))
}
