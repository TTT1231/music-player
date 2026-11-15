//! 验收：重启后恢复播放列表及顺序、上次的歌曲（暂停状态）、播放模式和音量。

mod common;

use common::{file, ids, playlist};
use music_core::{PlayMode, Player, load_session, save_session};

fn used_player() -> Player {
    let list = playlist(&["a - 一", "a - 二", "a - 三"]);
    let ids = ids(&list);
    let mut p = Player::new(list);
    p.import([file("a - 二.lrc")]);
    p.move_track(2, 0);
    p.play_track(&ids[1]);
    p.set_mode(PlayMode::Single);
    p.set_volume(0.35);
    p.set_rate(1.3);
    p.set_pitch(2);
    p
}

#[test]
fn saves_and_restores_list_current_track_mode_and_volume_paused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.json");
    let before = used_player();

    save_session(&path, &before.session()).unwrap();
    let restored = Player::restore(load_session(&path).unwrap().expect("已保存"));

    assert_eq!(restored.playlist(), before.playlist());
    assert_eq!(restored.current(), before.current());
    assert_eq!(restored.mode(), PlayMode::Single);
    assert_eq!(restored.volume(), 0.35);
    assert!(!restored.is_playing());
}

#[test]
fn rate_and_pitch_are_not_restored() {
    let restored = Player::restore(used_player().session());
    assert_eq!(restored.rate(), 1.0);
    assert_eq!(restored.pitch(), 0);
}

#[test]
fn missing_file_loads_as_none() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(load_session(&dir.path().join("session.json")).unwrap(), None);
}

#[test]
fn saving_again_replaces_the_file_and_leaves_no_temp_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.json");
    let mut p = used_player();

    save_session(&path, &p.session()).unwrap();
    p.set_volume(0.9);
    save_session(&path, &p.session()).unwrap();

    assert_eq!(load_session(&path).unwrap().unwrap().volume, 0.9);
    let names: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, ["session.json"]);
}
