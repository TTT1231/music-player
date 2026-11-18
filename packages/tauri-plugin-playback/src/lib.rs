//! 播放插件：安卓前台服务与 MediaSession 通知、音频焦点与耳机拔出事件、
//! SAF 文件选择与按 URI 打开文件。文件选择和打开只供外壳（Rust）调用。

use tauri::{
    Manager, Runtime,
    plugin::{Builder, TauriPlugin},
};

#[cfg(desktop)]
use desktop::Playback;
#[cfg(mobile)]
use mobile::Playback;

mod commands;
mod models;

pub use models::DocumentFile;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[cfg(mobile)]
    #[error(transparent)]
    PluginInvoke(#[from] tauri::plugin::mobile::PluginInvokeError),
}

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.to_string().as_str())
    }
}

/// 打开系统文件选择器选择歌曲和歌词（可多选）；取消时返回空列表。
#[cfg(mobile)]
pub fn pick_files<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<Vec<DocumentFile>> {
    app.state::<Playback<R>>().pick("pickFiles")
}

/// 选择一个文件夹，返回其中（含子文件夹）的全部文件；取消时返回空列表。
#[cfg(mobile)]
pub fn pick_folder<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<Vec<DocumentFile>> {
    app.state::<Playback<R>>().pick("pickFolder")
}

/// 按内容 URI 打开文件，返回原始文件描述符（调用方负责关闭）。
#[cfg(mobile)]
pub fn open_fd<R: Runtime>(app: &tauri::AppHandle<R>, uri: &str) -> Result<i32> {
    app.state::<Playback<R>>().open_fd(uri.to_owned())
}

/// 删除已安全落入播放器曲库的源文档。文件提供程序可以拒绝删除。
#[cfg(mobile)]
pub fn delete_source<R: Runtime>(app: &tauri::AppHandle<R>, uri: &str) -> Result<()> {
    app.state::<Playback<R>>().delete_source(uri.to_owned())
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("playback")
        .invoke_handler(tauri::generate_handler![
            commands::notify_state,
            commands::start_events,
            commands::move_to_background
        ])
        .setup(|app, api| {
            #[cfg(mobile)]
            let playback = mobile::Playback::init(app, api)?;
            #[cfg(desktop)]
            let playback = desktop::Playback::init(app, api)?;
            app.manage(playback);
            Ok(())
        })
        .build()
}

#[cfg(desktop)]
mod desktop {
    use tauri::{AppHandle, Runtime, plugin::PluginApi};

    #[derive(Debug)]
    pub struct Playback<R: Runtime>(std::marker::PhantomData<fn() -> R>);

    impl<R: Runtime> Playback<R> {
        pub fn init(_app: &AppHandle<R>, _api: PluginApi<R, ()>) -> crate::Result<Playback<R>> {
            Ok(Playback(std::marker::PhantomData))
        }
    }
}

#[cfg(mobile)]
mod mobile {
    use tauri::{AppHandle, Runtime, plugin::PluginApi};

    use crate::models::*;

    const PLUGIN_IDENTIFIER: &str = "app.tauri.playback";
    /// Kotlin trigger 的事件名（与基类 registerListener 通道对应）
    const PLAYBACK_EVENT: &str = "playbackEvent";

    #[derive(Debug)]
    pub struct Playback<R: Runtime>(tauri::plugin::PluginHandle<R>);

    impl<R: Runtime> Playback<R> {
        pub fn init(_app: &AppHandle<R>, api: PluginApi<R, ()>) -> crate::Result<Playback<R>> {
            Ok(Playback(
                api.register_android_plugin(PLUGIN_IDENTIFIER, "PlaybackPlugin")?,
            ))
        }

        pub fn pick(&self, method: &str) -> crate::Result<Vec<DocumentFile>> {
            Ok(self.0.run_mobile_plugin::<PickResponse>(method, ())?.files)
        }

        pub fn open_fd(&self, uri: String) -> crate::Result<i32> {
            Ok(self.0.run_mobile_plugin::<FdResponse>("openFd", UriPayload { uri })?.fd)
        }

        pub fn delete_source(&self, uri: String) -> crate::Result<()> {
            Ok(self.0.run_mobile_plugin::<()>("deleteSource", UriPayload { uri })?)
        }

        pub fn update_state(&self, state: StatePayload) -> crate::Result<()> {
            Ok(self.0.run_mobile_plugin::<()>("updateState", state)?)
        }

        pub fn move_to_background(&self) -> crate::Result<()> {
            Ok(self.0.run_mobile_plugin::<()>("moveToBackground", ())?)
        }

        pub fn register_events(&self, channel: tauri::ipc::Channel<serde_json::Value>) -> crate::Result<()> {
            let payload = RegisterListenerPayload {
                event: PLAYBACK_EVENT.to_owned(),
                handler: channel,
            };
            Ok(self.0.run_mobile_plugin::<()>("registerPlaybackListener", payload)?)
        }
    }
}
