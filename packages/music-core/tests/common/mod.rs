#![allow(dead_code)]

use std::path::PathBuf;

use music_core::{FileRef, Library, TrackId};

/// 仓库根的 fixtures/music/，由 scripts/gen-fixtures.mjs 生成。
pub fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/music")
}

/// 只有文件名的 FileRef，位置用 `/music/<文件名>` 代替，不需要真实文件。
pub fn file(name: &str) -> FileRef {
    FileRef::new(format!("/music/{name}"), name)
}

/// 由若干歌曲文件名组成的曲库（都没有歌词）。
pub fn library(names: &[&str]) -> Library {
    let mut library = Library::new();
    library.import(names.iter().map(|n| file(&format!("{n}.mp3"))));
    library
}

pub fn ids(library: &Library) -> Vec<TrackId> {
    library.tracks().iter().map(|t| t.id.clone()).collect()
}

pub fn titles(library: &Library) -> Vec<String> {
    library.tracks().iter().map(|t| t.title.clone()).collect()
}
