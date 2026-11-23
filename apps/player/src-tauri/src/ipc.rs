//! Tauri 命令只负责参数转换并转发给控制器线程。

use std::path::Path;
use std::sync::{Condvar, Mutex};

use music_audio::Cover;
use music_core::{Lyrics, PlayMode};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
#[cfg(desktop)]
use tauri_plugin_dialog::DialogExt;

use crate::actor::Actor;
use crate::audio_backend::{AudioAvailability, DeviceBackend};
use crate::controller::{AudioBackend, Controller, ImportResult, LocationResolver, PlayerState};
use crate::cover::CoverCache;
use crate::library_files::LibraryFiles;
#[cfg(desktop)]
use crate::library_files::files_from_paths;

pub type DeviceActor = Actor<DeviceBackend>;

#[derive(Default)]
struct CoverState {
    cache: CoverCache,
    revision: u64,
    imports: usize,
}

#[derive(Default)]
pub struct CoverStore {
    state: Mutex<CoverState>,
    idle: Condvar,
}

pub fn get_cover<B: AudioBackend + 'static>(actor: &Actor<B>, store: &CoverStore, id: String) -> Option<Cover> {
    let resolver = actor.call(|controller| controller.resolver()).ok()?;
    get_cover_with(actor, store, id, move |path| load_cover(&resolver, path))
}

/// 内嵌封面优先；没有时桌面端回退到曲库里与歌曲同名的图片。
pub fn load_cover(resolver: &LocationResolver, path: &Path) -> Option<Cover> {
    let (file, ext) = resolver(path.to_str()?).ok()?;
    let embedded = music_audio::probe_file(file, ext.as_deref()).ok()?.cover;
    #[cfg(desktop)]
    let embedded = embedded.or_else(|| crate::cover::sidecar_image(path));
    embedded
}

/// 缓存锁不跨越文件读取；若导入改变了歌曲，丢弃旧结果并重新查询。
pub fn get_cover_with<B: AudioBackend + 'static>(
    actor: &Actor<B>,
    store: &CoverStore,
    id: String,
    load: impl Fn(&Path) -> Option<Cover>,
) -> Option<Cover> {
    loop {
        let mut state = store.state.lock().unwrap_or_else(|e| e.into_inner());
        while state.imports > 0 {
            state = store.idle.wait(state).unwrap_or_else(|e| e.into_inner());
        }
        let revision = state.revision;
        drop(state);

        let lookup = id.clone();
        let path = actor.call(move |controller| controller.cover_path(&lookup)).ok()?;
        let state = store.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.revision != revision || state.imports > 0 {
            continue;
        }
        let path = path?;
        if let Some(cached) = state.cache.lookup(&id, &path) {
            return cached;
        }
        drop(state);

        let cover = load(&path);
        let mut state = store.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.revision != revision || state.imports > 0 {
            continue;
        }
        state.cache.insert(&id, &path, cover.clone());
        return cover;
    }
}

/// 导入桌面路径（文件或文件夹）。
pub fn import_files<B: AudioBackend + 'static>(
    actor: &Actor<B>,
    store: &CoverStore,
    paths: Vec<String>,
) -> Result<ImportResult, String> {
    import_with(actor, store, move |controller| {
        controller.import_paths(&paths).map_err(|e| e.to_string())
    })
}

/// 在控制器线程执行一次导入，期间让封面缓存失效（同名歌曲可能换了文件）。
pub fn import_with<B, F>(actor: &Actor<B>, store: &CoverStore, job: F) -> Result<ImportResult, String>
where
    B: AudioBackend + 'static,
    F: FnOnce(&mut Controller<B>) -> Result<ImportResult, String> + Send + 'static,
{
    {
        let mut state = store.state.lock().unwrap_or_else(|e| e.into_inner());
        state.imports += 1;
        state.revision = state.revision.wrapping_add(1);
        state.cache.clear();
    }
    let result = actor.call(job).and_then(|result| result);
    {
        let mut state = store.state.lock().unwrap_or_else(|e| e.into_inner());
        state.imports -= 1;
        state.revision = state.revision.wrapping_add(1);
        state.cache.clear();
        store.idle.notify_all();
    }
    result
}

#[derive(Serialize)]
pub struct AppInfo {
    platform: &'static str,
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        platform: if cfg!(target_os = "android") {
            "android"
        } else {
            "desktop"
        },
    }
}

#[tauri::command(async)]
pub fn get_state(actor: State<'_, DeviceActor>) -> Result<PlayerState, String> {
    actor.call(|controller| controller.state())
}

#[tauri::command(async)]
pub fn import_paths(app: AppHandle, paths: Vec<String>) -> Result<ImportResult, String> {
    #[cfg(desktop)]
    {
        let files = files_from_paths(&paths).map_err(|error| error.to_string())?;
        import_managed(&app, files, |location| std::fs::remove_file(location))
    }
    #[cfg(not(desktop))]
    {
        let _ = (app, paths);
        Err("请通过系统文件选择器导入".into())
    }
}

fn import_managed(
    app: &AppHandle,
    files: Vec<music_core::FileRef>,
    remove_source: impl Fn(&str) -> std::io::Result<()> + Send + 'static,
) -> Result<ImportResult, String> {
    let library = app.state::<LibraryFiles>().inner().clone();
    import_with(
        &app.state::<DeviceActor>(),
        &app.state::<CoverStore>(),
        move |controller| {
            controller
                .import_managed(files, &library, remove_source)
                .map_err(|error| error.to_string())
        },
    )
}

#[tauri::command]
pub async fn pick_and_import(app: AppHandle, kind: String) -> Result<Option<ImportResult>, String> {
    #[cfg(desktop)]
    {
        let chosen = match kind.as_str() {
            "files" => app.dialog().file().blocking_pick_files(),
            "folder" => app.dialog().file().blocking_pick_folder().map(|folder| vec![folder]),
            _ => return Err("未知的导入方式".into()),
        };
        let Some(chosen) = chosen else {
            return Ok(None);
        };
        let paths = chosen
            .into_iter()
            .map(|path| {
                path.into_path()
                    .map(|path| path.to_string_lossy().into_owned())
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        import_paths(app, paths).map(Some)
    }
    // 安卓：系统文件选择器返回内容 URI 和显示名；URI 本身不含文件名，配对和歌名都要用显示名
    #[cfg(mobile)]
    {
        let picked = match kind.as_str() {
            "files" => tauri_plugin_playback::pick_files(&app),
            "folder" => tauri_plugin_playback::pick_folder(&app),
            _ => return Err("未知的导入方式".into()),
        }
        .map_err(|error| error.to_string())?;
        if picked.is_empty() {
            return Ok(None);
        }
        let files: Vec<_> = picked
            .into_iter()
            .map(|file| music_core::FileRef::new(file.uri, file.name))
            .collect();
        let handle = app.clone();
        import_managed(&app, files, move |uri| {
            tauri_plugin_playback::delete_source(&handle, uri).map_err(std::io::Error::other)
        })
        .map(Some)
    }
}

/// 打开设备与操作在同一条消息内执行，避免空闲设备在两者之间被释放。
fn with_output<T: Send + 'static>(
    actor: &DeviceActor,
    audio: &AudioAvailability,
    action: impl FnOnce(&mut Controller<DeviceBackend>) -> T + Send + 'static,
) -> Result<T, String> {
    let audio = audio.clone();
    actor.call(move |controller| {
        controller.recover_output();
        audio.require()?;
        Ok(action(controller))
    })?
}

#[tauri::command(async)]
pub fn play_track(
    actor: State<'_, DeviceActor>,
    audio: State<'_, AudioAvailability>,
    id: String,
) -> Result<(), String> {
    with_output(&actor, &audio, move |c| c.play_track(&id))
}
#[tauri::command(async)]
pub fn remove_track(actor: State<'_, DeviceActor>, library: State<'_, LibraryFiles>, id: String) -> Result<(), String> {
    let library = library.inner().clone();
    actor
        .call(move |c| c.remove_managed(&id, &library))?
        .map_err(|error| error.to_string())
}
#[tauri::command(async)]
pub fn play(actor: State<'_, DeviceActor>, audio: State<'_, AudioAvailability>) -> Result<(), String> {
    with_output(&actor, &audio, Controller::play)
}
#[tauri::command(async)]
pub fn pause(actor: State<'_, DeviceActor>) -> Result<(), String> {
    actor.call(|c| c.pause())
}
#[tauri::command(async)]
pub fn toggle(actor: State<'_, DeviceActor>, audio: State<'_, AudioAvailability>) -> Result<(), String> {
    with_output(&actor, &audio, Controller::toggle)
}
#[tauri::command(async)]
pub fn next(actor: State<'_, DeviceActor>, audio: State<'_, AudioAvailability>) -> Result<(), String> {
    with_output(&actor, &audio, Controller::next)
}
#[tauri::command(async)]
pub fn prev(actor: State<'_, DeviceActor>, audio: State<'_, AudioAvailability>) -> Result<(), String> {
    with_output(&actor, &audio, Controller::prev)
}
#[tauri::command(async)]
pub fn seek(actor: State<'_, DeviceActor>, audio: State<'_, AudioAvailability>, ms: u64) -> Result<(), String> {
    with_output(&actor, &audio, move |c| c.seek(ms))?
}
#[tauri::command(async)]
pub fn set_mode(actor: State<'_, DeviceActor>, mode: PlayMode) -> Result<(), String> {
    actor.call(move |c| c.set_mode(mode))
}
#[tauri::command(async)]
pub fn set_volume(actor: State<'_, DeviceActor>, volume: f32) -> Result<(), String> {
    actor.call(move |c| c.set_volume(volume))
}
#[tauri::command(async)]
pub fn set_rate(actor: State<'_, DeviceActor>, rate: f32) -> Result<f32, String> {
    actor.call(move |c| c.set_rate(rate))
}
#[tauri::command(async)]
pub fn set_pitch(actor: State<'_, DeviceActor>, semitones: i8) -> Result<i8, String> {
    actor.call(move |c| c.set_pitch(semitones))
}
#[tauri::command(async)]
pub fn move_track(actor: State<'_, DeviceActor>, from: usize, to: usize) -> Result<(), String> {
    actor.call(move |c| c.move_track(from, to))
}

#[derive(Serialize)]
pub struct LyricView {
    #[serde(rename = "timeMs")]
    time_ms: u64,
    text: String,
}
#[derive(Serialize)]
pub struct LyricsResponse {
    lines: Vec<LyricView>,
}
pub fn lyrics_response(lyrics: Lyrics) -> LyricsResponse {
    LyricsResponse {
        lines: lyrics
            .lines
            .into_iter()
            .map(|line| LyricView {
                time_ms: line.time_ms,
                text: line.text,
            })
            .collect(),
    }
}
#[tauri::command(async)]
pub fn get_lyrics(actor: State<'_, DeviceActor>, id: String) -> Result<LyricsResponse, String> {
    actor
        .call(move |c| c.lyrics(&id))?
        .map(lyrics_response)
        .map_err(|e| e.to_string())
}

// Windows 上同步命令里创建 WebviewWindow 会死锁（webview 停在 about:blank），必须用 async 命令
#[tauri::command]
pub async fn toggle_desktop_lyrics(app: AppHandle) -> Result<bool, String> {
    #[cfg(desktop)]
    return crate::desktop_lyrics::toggle(&app);
    #[cfg(not(desktop))]
    {
        let _ = app;
        Err("桌面歌词只在桌面端可用".into())
    }
}
