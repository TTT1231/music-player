//! 前端可调用的插件命令：同步通知栏状态、开启原生事件转发。安卓以外的平台为空操作。

use tauri::{AppHandle, Runtime, State};

use crate::Playback;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn notify_state<R: Runtime>(
    // 只用于让命令宏推断运行时类型
    _app: AppHandle<R>,
    state: State<'_, Playback<R>>,
    playing: bool,
    title: Option<String>,
    artist: Option<String>,
    duration_ms: Option<u64>,
    position_ms: u64,
    rate: f32,
) -> crate::Result<()> {
    let payload = crate::models::StatePayload {
        playing,
        title,
        artist,
        duration_ms,
        position_ms,
        rate,
    };
    #[cfg(mobile)]
    return state.update_state(payload);
    #[cfg(desktop)]
    {
        let _ = (&state, payload);
        Ok(())
    }
}

/// 把应用退到后台（同按主页键）；系统返回键在首页时代替关闭应用，关闭会结束进程、音乐随之停止。
#[tauri::command]
pub(crate) async fn move_to_background<R: Runtime>(
    // 只用于让命令宏推断运行时类型
    _app: AppHandle<R>,
    state: State<'_, Playback<R>>,
) -> crate::Result<()> {
    #[cfg(mobile)]
    return state.move_to_background();
    #[cfg(desktop)]
    {
        let _ = state;
        Ok(())
    }
}

/// 注册原生事件（焦点丢失、耳机拔出、通知按钮）的转发：以 `playback://event` 事件发给前端。
#[tauri::command]
pub(crate) async fn start_events<R: Runtime>(app: AppHandle<R>, state: State<'_, Playback<R>>) -> crate::Result<()> {
    #[cfg(mobile)]
    {
        use tauri::Emitter;
        use tauri::ipc::{Channel, InvokeResponseBody};
        let channel = Channel::new(move |event: InvokeResponseBody| {
            let payload = match event {
                InvokeResponseBody::Json(json) => serde_json::from_str(&json).unwrap_or(serde_json::Value::Null),
                _ => serde_json::Value::Null,
            };
            if let Err(error) = app.emit("playback://event", payload) {
                log::error!("转发播放事件失败：{error}");
            }
            Ok(())
        });
        state.register_events(channel)
    }
    #[cfg(desktop)]
    {
        let _ = (app, state);
        Ok(())
    }
}
