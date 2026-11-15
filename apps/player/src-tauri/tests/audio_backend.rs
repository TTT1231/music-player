use std::path::Path;

use music_player_shell_lib::audio_backend::DeviceBackend;
use music_player_shell_lib::controller::AudioBackend;

#[test]
fn unavailable_device_reports_the_error_without_claiming_playback() {
    let mut device = DeviceBackend::unavailable("没有可用的音频输出设备".into());
    assert!(!device.available());
    assert!(
        device
            .load(Path::new("unused.mp3"))
            .unwrap_err()
            .contains("音频输出设备")
    );
}
