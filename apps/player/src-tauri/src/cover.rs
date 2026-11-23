//! 本地封面缓存与协议标识编码。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use music_audio::{Cover, probe};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use tauri::http::{self, header};

pub fn encode_cover_id(id: &str) -> String {
    utf8_percent_encode(id, NON_ALPHANUMERIC).to_string()
}

pub fn decode_cover_id(encoded: &str) -> Option<String> {
    percent_decode_str(encoded).decode_utf8().ok().map(|id| id.into_owned())
}

/// 封面可用于跨源 canvas；同名导入后 WebView 不应继续使用旧响应。
pub fn cover_response(cover: Option<Cover>) -> http::Response<Vec<u8>> {
    let mut response = http::Response::builder()
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CACHE_CONTROL, "no-store");
    let body = match cover {
        Some(cover) => {
            let mime = header::HeaderValue::from_str(&cover.mime)
                .unwrap_or_else(|_| header::HeaderValue::from_static("application/octet-stream"));
            response = response.status(200).header(header::CONTENT_TYPE, mime);
            cover.data
        }
        None => {
            response = response.status(404);
            Vec::new()
        }
    };
    response.body(body).expect("封面响应头有效")
}

#[derive(Default)]
pub struct CoverCache {
    covers: HashMap<(String, PathBuf), Option<Cover>>,
}

impl CoverCache {
    pub fn lookup(&self, id: &str, path: &Path) -> Option<Option<Cover>> {
        self.covers.get(&(id.to_string(), path.to_path_buf())).cloned()
    }

    pub fn insert(&mut self, id: &str, path: &Path, cover: Option<Cover>) {
        self.covers.insert((id.to_string(), path.to_path_buf()), cover);
    }

    pub fn get(&mut self, id: &str, path: &Path) -> Option<Cover> {
        self.covers
            .entry((id.to_string(), path.to_path_buf()))
            .or_insert_with(|| probe(path).ok().and_then(|media| media.cover))
            .clone()
    }

    pub fn clear(&mut self) {
        self.covers.clear();
    }
}

/// 曲库中与音频同名的图片（桌面）：`<曲库>/assets/music/<曲>.mp3` 找 `<曲库>/assets/img/<同名>`，
/// 旧会话的外部音频找同级 `img/`。文件名比较去首尾空白、忽略大小写。
#[cfg(desktop)]
pub fn sidecar_image(audio: &Path) -> Option<Cover> {
    sidecar_paths(audio).into_iter().find_map(|path| {
        let data = fs::read(&path).ok()?;
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        let mime = music_audio::sniff_image_mime(&data)
            .map(String::from)
            .or_else(|| match ext.as_str() {
                "jpg" | "jpeg" => Some("image/jpeg".into()),
                "png" => Some("image/png".into()),
                _ => None,
            })?;
        Some(Cover { mime, data })
    })
}

/// 音频对应的同名图片实际存在的路径，曲库布局在前、外部同级布局在后。
#[cfg(desktop)]
pub fn sidecar_paths(audio: &Path) -> Vec<PathBuf> {
    let stem = audio
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::trim)
        .filter(|stem| !stem.is_empty());
    let Some(stem) = stem else {
        return Vec::new();
    };
    let Some(parent) = audio.parent().filter(|parent| !parent.as_os_str().is_empty()) else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    if let Some(base) = parent.parent() {
        dirs.push(base.join("img"));
    }
    dirs.push(parent.join("img"));
    let wanted = stem.to_lowercase();
    dirs.into_iter()
        .find_map(|dir| find_image_file(&dir, &wanted))
        .into_iter()
        .collect()
}

/// 在目录里找“去扩展名、trim、忽略大小写后等于 wanted”的 jpg/jpeg/png。
#[cfg(desktop)]
pub(crate) fn find_image_file(dir: &Path, wanted: &str) -> Option<PathBuf> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some((stem, ext)) = name.rsplit_once('.') else {
            continue;
        };
        if !matches!(ext.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png") {
            continue;
        }
        if stem.trim().eq_ignore_ascii_case(wanted) {
            return Some(entry.path());
        }
    }
    None
}
