pub mod actor;
pub mod audio_backend;
pub mod controller;
pub mod cover;
pub mod ipc;

use std::path::PathBuf;

use tauri::{Emitter, Manager};

use actor::{Actor, Projection};
use audio_backend::{AudioAvailability, DeviceBackend};
use cover::{cover_response, decode_cover_id};
use ipc::{CoverStore, DeviceActor};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("cover", |ctx, request, responder| {
            let id = decode_cover_id(request.uri().path().trim_start_matches('/'));
            let app = ctx.app_handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                let cover =
                    id.and_then(|id| ipc::get_cover(&app.state::<DeviceActor>(), &app.state::<CoverStore>(), id));
                responder.respond(cover_response(cover));
            });
        })
        .setup(|app| {
            let data_dir = match std::env::var_os("MUSIC_DATA_DIR") {
                Some(path) => PathBuf::from(path),
                None => app.path().app_data_dir()?,
            };
            std::fs::create_dir_all(&data_dir)?;
            let handle = app.handle().clone();
            let availability = AudioAvailability::default();
            let output_availability = availability.clone();
            let event_availability = availability.clone();
            let actor = Actor::spawn(
                data_dir.join("session.json"),
                move |callback| {
                    let output = DeviceBackend::open(callback, output_availability);
                    Ok(output)
                },
                move |event| {
                    let result = match event {
                        Projection::State(state) => handle.emit("state", state),
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
            ipc::import_paths,
            ipc::pick_and_import,
            ipc::play_track,
            ipc::remove_track,
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
            ipc::get_lyrics,
        ])
        .build(tauri::generate_context!())
        .expect("无法启动音乐播放器")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<DeviceActor>().shutdown();
            }
        });
}
