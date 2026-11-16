//! 桌面歌词窗口：只在桌面端编译；负责创建/关闭 label 为 `lyrics` 的第二个窗口。

use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "lyrics";
const WIDTH: f64 = 980.0;

/// 打开（返回 true）或关闭（返回 false）桌面歌词窗口。
pub fn toggle<R: Runtime>(app: &AppHandle<R>) -> Result<bool, String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        window.close().map_err(|error| error.to_string())?;
        return Ok(false);
    }
    let builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("桌面歌词")
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .inner_size(WIDTH, 160.0)
        .center();
    builder.build().map_err(|error| error.to_string())?;
    Ok(true)
}
