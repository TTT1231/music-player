const COMMANDS: &[&str] = &["notify_state", "start_events", "move_to_background"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).android_path("android").build();
}
