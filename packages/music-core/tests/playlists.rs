//! 歌单：曲库、自建歌单、播放列表三者分开。
//! 从歌单点播时播放列表整份替换；移出播放列表不动歌单和曲库；移出歌单不动播放列表；
//! 从曲库移除时三处一起清掉；重启后恢复播放列表和歌单，旧会话按曲库顺序升级。

mod common;

use std::collections::HashSet;
use std::time::{Duration, SystemTime};

use common::{file, ids, library};
use music_core::{
    ALL_MUSIC_NAME, MAX_PLAYLIST_NAME, Player, Playlist, PlaylistError, PlaylistId, Playlists, Step, TrackId,
    load_session, save_session,
};

/// 曲库一、二、三；歌单“夜跑”为 二、三；播放列表为 三、二、一，正在播放 三。
fn arranged() -> (Player, Vec<TrackId>, PlaylistId) {
    let mut p = Player::new(library(&["a - 一", "a - 二", "a - 三"]));
    let ids = ids(p.library());
    let night = p.create_playlist("夜跑").unwrap();
    p.add_to_playlist(&night, &[ids[1].clone(), ids[2].clone()]).unwrap();
    p.play_from(&[ids[2].clone(), ids[1].clone(), ids[0].clone()], &ids[2])
        .unwrap();
    (p, ids, night)
}

#[test]
fn a_new_player_queues_the_whole_library_and_has_no_playlists() {
    let p = Player::new(library(&["a - 一", "a - 二"]));
    assert_eq!(p.queue(), ids(p.library()));
    assert_eq!(p.playlists().len(), 0);
}

#[test]
fn playing_from_a_list_replaces_the_queue_in_that_order() {
    let (mut p, ids, _) = arranged();
    assert_eq!(p.queue(), [ids[2].clone(), ids[1].clone(), ids[0].clone()]);
    assert_eq!(p.current(), Some(&ids[2]));
    assert!(p.is_playing());
    // 播完按新顺序接下一首，列表循环
    assert_eq!(p.on_ended(), Step::Play(ids[1].clone()));
    assert_eq!(p.on_ended(), Step::Play(ids[0].clone()));
    assert_eq!(p.on_ended(), Step::Play(ids[2].clone()));
    // 曲库还是导入顺序
    assert_eq!(p.library().len(), 3);
    assert_eq!(common::ids(p.library()), ids);
}

#[test]
fn play_from_skips_unknown_songs_and_rejects_a_start_outside_the_list() {
    let (mut p, ids, _) = arranged();
    let unknown = TrackId("a - 不存在".into());
    assert_eq!(
        p.play_from(&[ids[0].clone(), unknown.clone()], &ids[0]),
        Some(Step::Play(ids[0].clone()))
    );
    assert_eq!(p.queue(), [ids[0].clone()]);
    // 起点不在列表里，或列表里没有曲库中存在的歌：什么都不变
    assert_eq!(p.play_from(std::slice::from_ref(&unknown), &unknown), None);
    assert_eq!(p.play_from(&[], &ids[1]), None);
    assert_eq!(p.current(), Some(&ids[0]));
    assert_eq!(p.queue(), [ids[0].clone()]);
}

#[test]
fn queue_playlist_and_library_are_removed_from_independently() {
    let (mut p, ids, night) = arranged();
    // 移出歌单：播放列表和曲库不动，正在播放的歌不受影响
    p.remove_from_playlist(&night, &ids[2]).unwrap();
    assert_eq!(p.playlists().get(&night).unwrap().tracks, [ids[1].clone()]);
    assert_eq!(p.current(), Some(&ids[2]));
    assert_eq!(p.queue().len(), 3);
    assert_eq!(p.library().len(), 3);
    // 歌单里没有的歌什么都不发生；歌单不存在才报错
    p.remove_from_playlist(&night, &ids[2]).unwrap();
    assert_eq!(
        p.remove_from_playlist(&PlaylistId("不存在".into()), &ids[2]),
        Err(PlaylistError::NotFound)
    );
    // 移出播放列表：歌单和曲库不动；移走的是正在播放的歌时接着播列表里的下一首
    assert_eq!(p.remove(&ids[2]), Some(Step::Play(ids[1].clone())));
    assert_eq!(p.queue(), [ids[1].clone(), ids[0].clone()]);
    assert_eq!(p.playlists().get(&night).unwrap().tracks, [ids[1].clone()]);
    assert_eq!(p.library().len(), 3);
}

#[test]
fn removing_from_the_library_clears_the_queue_and_every_playlist() {
    let (mut p, ids, night) = arranged();
    assert_eq!(p.remove_from_library(&ids[2]), Some(Step::Play(ids[1].clone())));
    assert_eq!(p.library().len(), 2);
    assert_eq!(p.queue(), [ids[1].clone(), ids[0].clone()]);
    assert_eq!(p.playlists().get(&night).unwrap().tracks, [ids[1].clone()]);
    // 不在曲库里的歌：什么都不发生
    assert_eq!(p.remove_from_library(&TrackId("a - 不存在".into())), None);
}

#[test]
fn enqueue_appends_library_songs_that_are_not_queued_yet() {
    let (mut p, ids, _) = arranged();
    p.play_from(std::slice::from_ref(&ids[1]), &ids[1]).unwrap();
    assert_eq!(
        p.enqueue([ids[1].clone(), ids[2].clone(), TrackId("a - 不存在".into())]),
        1
    );
    assert_eq!(p.queue(), [ids[1].clone(), ids[2].clone()]);
}

#[test]
fn importing_does_not_touch_the_queue() {
    let (mut p, ids, _) = arranged();
    p.play_from(std::slice::from_ref(&ids[0]), &ids[0]).unwrap();
    p.import([file("a - 四.mp3")]);
    assert_eq!(p.library().len(), 4);
    assert_eq!(p.queue(), [ids[0].clone()]);
}

#[test]
fn playlist_names_are_trimmed_unique_and_never_the_builtin_one() {
    let mut p = Player::new(library(&["a - 一"]));
    let night = p.create_playlist("  夜跑 ").unwrap();
    assert_eq!(p.playlists().get(&night).unwrap().name, "夜跑");
    assert_eq!(p.create_playlist(" 夜跑"), Err(PlaylistError::DuplicateName));
    // 重名不区分大小写
    p.create_playlist("Night").unwrap();
    assert_eq!(p.create_playlist(" nIGHT "), Err(PlaylistError::DuplicateName));
    assert_eq!(p.create_playlist("   "), Err(PlaylistError::EmptyName));
    assert_eq!(p.create_playlist(ALL_MUSIC_NAME), Err(PlaylistError::ReservedName));
    let too_long = "长".repeat(MAX_PLAYLIST_NAME + 1);
    assert_eq!(p.create_playlist(&too_long), Err(PlaylistError::NameTooLong));
    assert!(p.create_playlist(&"长".repeat(MAX_PLAYLIST_NAME)).is_ok());
}

#[test]
fn playlist_ids_stay_unique() {
    let mut p = Player::new(library(&["a - 一"]));
    // 连续创建、删掉再建的歌单不会复用 id
    let mut created = Vec::new();
    for _ in 0..50 {
        let id = p.create_playlist("临时").unwrap();
        p.delete_playlist(&id).unwrap();
        created.push(id);
    }
    let unique: HashSet<&PlaylistId> = created.iter().collect();
    assert_eq!(unique.len(), 50);
}

#[test]
fn playlist_ids_stay_unique_for_the_same_clock_reading() {
    let mut playlists = Playlists::default();
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let first = playlists.create("一", now).unwrap();
    let second = playlists.create("二", now).unwrap();
    assert_ne!(first, second);
}

#[test]
fn renaming_keeps_the_own_name_and_rejects_another_playlists_name() {
    let (mut p, _, night) = arranged();
    p.create_playlist("白天").unwrap();
    p.rename_playlist(&night, "夜跑").unwrap();
    assert_eq!(p.rename_playlist(&night, " 白天 "), Err(PlaylistError::DuplicateName));
    assert_eq!(
        p.rename_playlist(&night, ALL_MUSIC_NAME),
        Err(PlaylistError::ReservedName)
    );
    p.rename_playlist(&night, " 深夜跑 ").unwrap();
    assert_eq!(p.playlists().get(&night).unwrap().name, "深夜跑");
    assert_eq!(
        p.rename_playlist(&PlaylistId("不存在".into()), "任意"),
        Err(PlaylistError::NotFound)
    );
}

#[test]
fn adding_songs_appends_in_the_given_order_without_duplicates_or_unknown_songs() {
    let (mut p, ids, night) = arranged();
    let added = p
        .add_to_playlist(
            &night,
            &[
                ids[0].clone(),
                ids[2].clone(),
                ids[0].clone(),
                TrackId("a - 不存在".into()),
            ],
        )
        .unwrap();
    assert_eq!(added, 1);
    assert_eq!(
        p.playlists().get(&night).unwrap().tracks,
        [ids[1].clone(), ids[2].clone(), ids[0].clone()]
    );
    assert_eq!(
        p.add_to_playlist(&PlaylistId("不存在".into()), &[ids[0].clone()]),
        Err(PlaylistError::NotFound)
    );
}

#[test]
fn deleting_a_playlist_leaves_the_queue_and_the_library_alone() {
    let (mut p, ids, night) = arranged();
    p.delete_playlist(&night).unwrap();
    assert_eq!(p.playlists().len(), 0);
    assert_eq!(p.queue().len(), 3);
    assert_eq!(p.current(), Some(&ids[2]));
    assert_eq!(p.library().len(), 3);
    assert_eq!(p.delete_playlist(&night), Err(PlaylistError::NotFound));
}

#[test]
fn session_restores_the_queue_order_and_the_playlists_paused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.json");
    let (p, ids, _) = arranged();
    save_session(&path, &p.session()).unwrap();

    let restored = Player::restore(load_session(&path).unwrap().expect("已保存"));

    assert_eq!(restored.queue(), [ids[2].clone(), ids[1].clone(), ids[0].clone()]);
    assert_eq!(restored.current(), Some(&ids[2]));
    assert!(!restored.is_playing());
    assert_eq!(restored.playlists(), p.playlists());
    assert_eq!(restored.library(), p.library());
}

#[test]
fn legacy_sessions_queue_the_whole_library_and_have_no_playlists() {
    // 最初的旧会话只有一份列表，既是曲库也是播放列表
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.json");
    let mut p = Player::new(library(&["a - 一", "a - 二", "a - 三"]));
    let ids = ids(p.library());
    p.play_track(&ids[1]);
    let mut legacy = serde_json::to_value(p.session()).unwrap();
    let fields = legacy.as_object_mut().unwrap();
    fields.remove("library");
    fields.remove("saved_playlists");
    fields.remove("queue_source");
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();

    let restored = Player::restore(load_session(&path).unwrap().expect("已保存"));

    assert_eq!(restored.queue(), ids);
    assert_eq!(restored.current(), Some(&ids[1]));
    assert_eq!(restored.playlists().len(), 0);
}

#[test]
fn sessions_written_by_the_earlier_android_build_still_load() {
    // 那一版把曲库写在 tracks 里、播放列表按 id 写在 queue 里、歌单写在 playlists 里
    let (p, ids, night) = arranged();
    let mut json = serde_json::to_value(p.session()).unwrap();
    let fields = json.as_object_mut().unwrap();
    let library = fields.remove("library").unwrap();
    fields.insert("tracks".into(), library);
    let queue = serde_json::to_value(ids[1..2].to_vec()).unwrap();
    fields.insert("queue".into(), queue);
    let mut playlists = fields.remove("saved_playlists").unwrap();
    for playlist in playlists.as_array_mut().unwrap() {
        let playlist = playlist.as_object_mut().unwrap();
        let tracks = playlist.remove("trackIds").unwrap();
        playlist.insert("tracks".into(), tracks);
    }
    fields.insert("playlists".into(), playlists);

    let restored = Player::restore(serde_json::from_value(json).unwrap());

    assert_eq!(restored.library().len(), 3);
    assert_eq!(restored.queue(), [ids[1].clone()]);
    assert_eq!(restored.current(), None);
    assert_eq!(
        restored.playlists().get(&night).unwrap().tracks,
        [ids[1].clone(), ids[2].clone()]
    );
}

#[test]
fn restore_keeps_an_empty_queue_and_drops_songs_missing_from_the_library() {
    let (p, ids, _) = arranged();
    let mut session = p.session();
    // 播放列表清空后重启仍是空的，不回落成整个曲库
    session.tracks = Vec::new();
    let restored = Player::restore(session.clone());
    assert!(restored.queue().is_empty());
    assert_eq!(restored.current(), None);
    assert_eq!(restored.library().len(), 3);

    // 会话里引用了曲库中已经没有的歌（手改过的会话文件）：忽略这些引用和重复项
    let gone = TrackId("a - 不存在".into());
    let mut ghost = p.track(&ids[0]).unwrap().clone();
    ghost.id = gone.clone();
    let old = PlaylistId("p旧".into());
    session.tracks = vec![p.track(&ids[1]).unwrap().clone(), ghost.clone(), ghost];
    session.saved_playlists = Playlists::from(vec![Playlist {
        id: old.clone(),
        name: "旧歌单".into(),
        tracks: vec![ids[0].clone(), gone.clone()],
    }]);
    session.current = Some(gone);
    let restored = Player::restore(session);

    assert_eq!(restored.queue(), [ids[1].clone()]);
    assert_eq!(restored.current(), None);
    assert_eq!(restored.playlists().get(&old).unwrap().tracks, [ids[0].clone()]);
}

#[test]
fn the_saved_file_keeps_the_desktop_layout_so_the_previous_desktop_build_can_read_it() {
    // 桌面歌单第一版写出的格式：tracks 是播放列表、library 是曲库、saved_playlists 的歌曲列表叫 trackIds
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.json");
    let (p, ids, _) = arranged();
    save_session(&path, &p.session()).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let fields = json.as_object().unwrap();
    let queued: Vec<&str> = fields["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|track| track["id"].as_str().unwrap())
        .collect();
    let order = [2, 1, 0].map(|at| ids[at].0.as_str());
    assert_eq!(queued, order);
    assert_eq!(fields["library"].as_array().unwrap().len(), 3);
    assert_eq!(fields["saved_playlists"][0]["trackIds"].as_array().unwrap().len(), 2);
    assert!(fields.get("queue").is_none() && fields.get("playlists").is_none());
    assert_eq!(fields["queue_source"], serde_json::Value::Null);
}
