//! 与 Kotlin 插件交换的数据。除 DocumentFile 外都只在安卓调用原生方法时用到。
#![cfg_attr(desktop, allow(dead_code))]

use serde::{Deserialize, Serialize};

/// 用户选中的一个文件：内容 URI 和显示名（含扩展名）。内容 URI 本身不含文件名。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentFile {
    pub uri: String,
    pub name: String,
}

/// 选择文件或文件夹的结果；用户取消时为空
#[derive(Debug, Deserialize)]
pub struct PickResponse {
    pub files: Vec<DocumentFile>,
}

#[derive(Debug, Serialize)]
pub struct UriPayload {
    pub uri: String,
}

#[derive(Debug, Deserialize)]
pub struct FdResponse {
    /// ParcelFileDescriptor.detachFd() 的原始描述符；Rust 侧负责关闭
    pub fd: i32,
}

/// 通知栏和锁屏卡片的播放状态；进度由系统按“位置 + 更新时间 + 倍速”推算，只在状态变化时发送
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatePayload {
    pub playing: bool,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub duration_ms: Option<u64>,
    pub position_ms: u64,
    pub rate: f32,
}

/// 与安卓基类 registerListener 的参数结构一致（event + handler）
#[derive(Serialize)]
pub struct RegisterListenerPayload {
    pub event: String,
    pub handler: tauri::ipc::Channel<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_response_carries_uri_and_display_name() {
        let json = r#"{"files":[{"uri":"content://media/document/audio%3A12","name":"歌手 - 歌名.mp3"}]}"#;
        let response: PickResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            response.files,
            [DocumentFile {
                uri: "content://media/document/audio%3A12".into(),
                name: "歌手 - 歌名.mp3".into()
            }]
        );
    }

    #[test]
    fn state_payload_carries_progress_for_the_lock_screen() {
        // 锁屏卡片按“位置 + 时间点 + 倍速”自行推算进度，需要时长、位置和倍速
        let payload = StatePayload {
            playing: true,
            title: Some("歌名".into()),
            artist: None,
            duration_ms: Some(180_000),
            position_ms: 5_000,
            rate: 1.2,
        };
        assert_eq!(
            serde_json::to_value(payload).unwrap(),
            serde_json::json!({
                "playing": true, "title": "歌名", "artist": null,
                "durationMs": 180_000, "positionMs": 5_000, "rate": 1.2_f32
            })
        );
    }

    #[test]
    fn cancelled_pick_is_an_empty_list() {
        let response: PickResponse = serde_json::from_str(r#"{"files":[]}"#).unwrap();
        assert!(response.files.is_empty());
    }
}
