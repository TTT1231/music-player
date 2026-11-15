//! 本地封面缓存与协议标识编码。

use std::collections::HashMap;
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
