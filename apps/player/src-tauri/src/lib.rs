pub mod actor;
pub mod audio_backend;
pub mod controller;
pub mod cover;
#[cfg(desktop)]
pub mod desktop_lyrics;
pub mod ipc;
pub mod library_files;

use std::path::PathBuf;

use tauri::{Emitter, Manager};

use actor::{Actor, Projection};
use audio_backend::{AudioAvailability, DeviceBackend};
use cover::{cover_response, decode_cover_id};
use ipc::{CoverStore, DeviceActor};
use library_files::LibraryFiles;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    // 桌面用系统文件对话框；安卓的文件选择由播放插件完成
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_dialog::init());
    builder
        .plugin(tauri_plugin_playback::init())
        .register_asynchronous_uri_scheme_protocol("cover", |ctx, request, responder| {
            let id = decode_cover_id(request.uri().path().trim_start_matches('/'));
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                let cover = id.and_then(|id| {
                    ipc::get_cover(
                        &app.state::<DeviceActor>(),
                        &app.state::<CoverStore>(),
                        &app.state::<LibraryFiles>(),
                        id,
                    )
                });
                responder.respond(cover_response(cover));
            });
        })
        .setup(|app| {
            let override_dir = std::env::var_os("MUSIC_DATA_DIR");
            let data_dir = match &override_dir {
                Some(path) => PathBuf::from(path),
                None => app.path().app_data_dir()?,
            };
            let library_base = if cfg!(target_os = "windows") && override_dir.is_none() {
                let executable = std::env::current_exe()?;
                executable.parent().ok_or("无法确定安装目录")?.to_path_buf()
            } else {
                data_dir.clone()
            };
            app.manage(LibraryFiles::new(library_base.join("assets")));
            std::fs::create_dir_all(&data_dir)?;
            let handle = app.handle().clone();
            let availability = AudioAvailability::default();
            let output_availability = availability.clone();
            let event_availability = availability.clone();
            // 桌面：位置即路径；安卓：content:// URI 经插件打开文件描述符
            #[cfg(target_os = "android")]
            let location_resolver = {
                use std::fs::File;
                use std::os::unix::io::FromRawFd;
                use std::path::PathBuf;
                let handle = app.handle().clone();
                let resolver: controller::LocationResolver =
                    std::sync::Arc::new(move |location: &str| -> std::io::Result<(File, Option<String>)> {
                        if location.starts_with("content://") {
                            let fd =
                                tauri_plugin_playback::open_fd(&handle, location).map_err(std::io::Error::other)?;
                            let file = unsafe { File::from_raw_fd(fd) };
                            // 内容 URI 不含文件名，不提供格式提示，由解码器按内容识别
                            Ok((file, None))
                        } else {
                            let path = PathBuf::from(location);
                            let ext = path.extension().and_then(|ext| ext.to_str()).map(str::to_owned);
                            Ok((File::open(&path)?, ext))
                        }
                    });
                resolver
            };
            #[cfg(not(target_os = "android"))]
            let location_resolver = controller::path_resolver();
            // 曲库（读封面、时长）和音频输出必须用同一个解析器：安卓的位置是 content:// URI，不能当路径打开
            let output_resolver = location_resolver.clone();
            let actor = Actor::spawn(
                data_dir.join("session.json"),
                location_resolver,
                move |callback| {
                    let output = DeviceBackend::new(callback, output_availability, output_resolver);
                    Ok(output)
                },
                move |event| {
                    let result = match event {
                        Projection::State(state) => handle.emit("state", state),
                        Projection::Library(library) => handle.emit("library", library),
                        Projection::Position { ms } => handle.emit("position", serde_json::json!({ "ms": ms })),
                        Projection::Notice { title } => {
                            handle.emit("notice", serde_json::json!({ "kind": "unreadable", "title": title }))
                        }
                        Projection::DeviceError(error) => {
                            event_availability.set_error(Some(error.clone()));
                            eprintln!("音频设备错误：{error}");
                            Ok(())
                        }
                    };
                    if let Err(error) = result {
                        eprintln!("发送播放器事件失败：{error}");
                    }
                },
            )
            .map_err(std::io::Error::other)?;
            app.manage(actor);
            app.manage(CoverStore::default());
            app.manage(availability);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::app_info,
            ipc::get_state,
            ipc::get_library,
            ipc::import_paths,
            ipc::pick_and_import,
            ipc::play_track,
            ipc::play_from,
            ipc::play_playlist,
            ipc::remove_track,
            ipc::remove_queue_track,
            ipc::play,
            ipc::pause,
            ipc::toggle,
            ipc::next,
            ipc::prev,
            ipc::seek,
            ipc::set_mode,
            ipc::set_volume,
            ipc::set_rate,
            ipc::set_pitch,
            ipc::move_track,
            ipc::create_playlist,
            ipc::rename_playlist,
            ipc::delete_playlist,
            ipc::add_playlist_tracks,
            ipc::remove_playlist_track,
            ipc::get_lyrics,
            ipc::toggle_desktop_lyrics,
        ])
        .build(tauri::generate_context!())
        .expect("无法启动音乐播放器")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<DeviceActor>().shutdown();
            }
        });
}
