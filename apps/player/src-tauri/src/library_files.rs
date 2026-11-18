//! 管理安装目录或应用数据目录中由播放器接管的文件。

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use music_core::{FileRef, Player, is_audio_file, scan_dir};

use crate::controller::LocationResolver;

#[derive(Clone)]
pub struct LibraryFiles {
    root: PathBuf,
}

impl LibraryFiles {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub(crate) fn unique_files(&self, player: &Player, mut files: Vec<FileRef>) -> (Vec<FileRef>, usize) {
        let mut names = HashSet::new();
        for track in player.playlist().tracks() {
            for location in std::iter::once(track.audio.as_str()).chain(track.lyrics.as_deref()) {
                let path = Path::new(location);
                if self.owns(path)
                    && path.is_file()
                    && let Some(ext) = path.extension().and_then(|ext| ext.to_str())
                {
                    names.insert(format!("{}.{}", track.id.0, ext.to_ascii_lowercase()));
                }
            }
        }

        let mut duplicates = 0;
        files.retain(|file| {
            if asset_folder(&file.file_name).is_none() {
                return false;
            }
            let (stem, ext) = file.file_name.rsplit_once('.').expect("supported extension");
            let new = names.insert(format!("{}.{}", stem.trim().to_lowercase(), ext.to_ascii_lowercase()));
            duplicates += usize::from(!new);
            new
        });
        (files, duplicates)
    }

    pub(crate) fn stage(
        &self,
        preview: &Player,
        mut files: Vec<FileRef>,
        open: &LocationResolver,
    ) -> io::Result<(Vec<FileRef>, StagedFiles)> {
        for name in ["music", "lysis"] {
            let folder = self.root.join(name);
            fs::create_dir_all(&folder).map_err(|error| {
                io::Error::new(error.kind(), format!("无法创建曲库目录 {}：{error}", folder.display()))
            })?;
        }
        let used = locations(preview);
        let mut staged = StagedFiles::default();
        for file in &mut files {
            let source = Path::new(&file.location);
            if !used.contains(file.location.as_str()) || (self.owns(source) && source.is_file()) {
                continue;
            }
            let folder = self
                .root
                .join(asset_folder(&file.file_name).expect("filtered file type"));
            let copy = || {
                let (mut input, _) = open(&file.location)?;
                let mut temp = tempfile::Builder::new().prefix(".import-").tempfile_in(&folder)?;
                io::copy(&mut input, temp.as_file_mut())?;
                temp.as_file().sync_all()?;
                let name = file.file_name.replace(['/', '\\', ':', '\0'], "_");
                persist_unique(temp, &folder, &name)
            };
            let saved = copy().map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("无法导入 {} 到 {}：{error}", file.file_name, folder.display()),
                )
            })?;
            let location = saved.to_string_lossy().into_owned();
            staged
                .files
                .push((std::mem::replace(&mut file.location, location), saved));
        }
        Ok((files, staged))
    }

    pub(crate) fn remove_unreferenced(
        &self,
        player: &Player,
        files: impl IntoIterator<Item = String>,
    ) -> io::Result<()> {
        let used = locations(player);
        let mut error = None;
        for location in files {
            if used.contains(location.as_str()) || !self.owns(Path::new(&location)) {
                continue;
            }
            if let Err(reason) = fs::remove_file(&location)
                && reason.kind() != io::ErrorKind::NotFound
            {
                error = Some(io::Error::new(
                    reason.kind(),
                    format!("已移出播放列表，无法删除 {location}：{reason}"),
                ));
            }
        }
        error.map_or(Ok(()), Err)
    }

    fn owns(&self, path: &Path) -> bool {
        let Some(parent) = path.parent() else { return false };
        let Some(folder @ ("music" | "lysis")) = parent.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        let owned = self.root.join(folder);
        parent == owned
            || parent
                .canonicalize()
                .is_ok_and(|actual| owned.canonicalize().is_ok_and(|dir| dir == actual))
    }
}

fn asset_folder(name: &str) -> Option<&'static str> {
    if is_audio_file(name) {
        Some("music")
    } else if name
        .rsplit_once('.')
        .is_some_and(|(stem, ext)| !stem.is_empty() && ext.eq_ignore_ascii_case("lrc"))
    {
        Some("lysis")
    } else {
        None
    }
}

fn locations(player: &Player) -> HashSet<&str> {
    player
        .playlist()
        .tracks()
        .iter()
        .flat_map(|track| std::iter::once(track.audio.as_str()).chain(track.lyrics.as_deref()))
        .collect()
}

fn persist_unique(mut file: tempfile::NamedTempFile, folder: &Path, name: &str) -> io::Result<PathBuf> {
    let (stem, ext) = name.rsplit_once('.').expect("supported extension");
    for number in 1.. {
        let path = if number == 1 {
            folder.join(name)
        } else {
            folder.join(format!("{stem} ({number}).{ext}"))
        };
        match file.persist_noclobber(&path) {
            Ok(_) => return Ok(path),
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => file = error.file,
            Err(error) => return Err(error.error),
        }
    }
    unreachable!()
}

#[derive(Default)]
pub(crate) struct StagedFiles {
    files: Vec<(String, PathBuf)>,
}

impl StagedFiles {
    pub(crate) fn commit(mut self, remove_source: impl Fn(&str) -> io::Result<()>) -> usize {
        self.files
            .drain(..)
            .filter(|(source, _)| remove_source(source).is_err_and(|error| error.kind() != io::ErrorKind::NotFound))
            .count()
    }
}

impl Drop for StagedFiles {
    fn drop(&mut self) {
        for (_, saved) in &self.files {
            let _ = fs::remove_file(saved);
        }
    }
}

pub(crate) fn files_from_paths(paths: &[String]) -> io::Result<Vec<FileRef>> {
    let mut files = Vec::new();
    for value in paths {
        let path = Path::new(value);
        if path.is_dir() {
            files.extend(scan_dir(path)?);
        } else {
            files.push(FileRef::from_path(path));
        }
    }
    Ok(files)
}
