mod common;

use common::{file, ids, library};
use music_core::{ALL_MUSIC_SOURCE, Player, PlaylistError, PlaylistId, Step, TrackId};

#[test]
fn saved_lists_are_distinct_from_library_and_queue_and_survive_restore() {
    let list = library(&["a - 一", "a - 二", "a - 三"]);
    let ids = ids(&list);
    let mut player = Player::new(list);
    let list_id = player.create_playlist("  喜欢  ").unwrap();
    assert!(player.create_playlist("喜欢").is_err());
    assert!(player.create_playlist("全部音乐").is_err());
    player
        .add_to_playlist(
            &list_id,
            &[
                ids[1].clone(),
                ids[0].clone(),
                ids[1].clone(),
                TrackId("missing".into()),
            ],
        )
        .unwrap();
    player.play_playlist(Some(&list_id), Some(&ids[1])).unwrap();
    assert_eq!(player.queue(), [ids[1].clone(), ids[0].clone()]);
    player.remove(&ids[1]);
    assert_eq!(player.library().len(), 3);
    assert_eq!(
        player.playlists().get(&list_id).unwrap().tracks,
        [ids[1].clone(), ids[0].clone()]
    );
    let restored = Player::restore(player.session());
    assert_eq!(restored.library().len(), 3);
    assert_eq!(restored.queue(), [ids[0].clone()]);
    assert_eq!(restored.playlists().get(&list_id).unwrap().tracks.len(), 2);
}

#[test]
fn removing_library_track_prunes_saved_lists_but_removing_from_list_does_not_touch_queue() {
    let list = library(&["a - 一", "a - 二"]);
    let ids = ids(&list);
    let mut player = Player::new(list);
    let list_id = player.create_playlist("歌单").unwrap();
    player.add_to_playlist(&list_id, &ids).unwrap();
    player.remove_from_playlist(&list_id, &ids[0]).unwrap();
    assert_eq!(player.queue().len(), 2);
    player.remove_from_library(&ids[1]);
    assert!(player.playlists().get(&list_id).unwrap().tracks.is_empty());
}

#[test]
fn queue_source_survives_edits_and_restart() {
    let mut player = Player::new(library(&["a - 一", "a - 二"]));
    let list_id = player.create_playlist("收藏").unwrap();
    let ids = ids(player.library());
    player.add_to_playlist(&list_id, &ids).unwrap();
    player.play_playlist(Some(&list_id), None).unwrap();
    player.remove(&ids[0]);
    assert_eq!(player.queue_source(), Some(list_id.0.as_str()));
    assert_eq!(
        Player::restore(player.session()).queue_source(),
        Some(list_id.0.as_str())
    );
}

#[test]
fn library_only_import_does_not_append_to_queue() {
    let mut player = Player::new(library(&["a - 一"]));
    player.import([file("b - 二.mp3")]);
    assert_eq!(player.library().len(), 2);
    assert_eq!(player.queue().len(), 1);
}

#[test]
fn playing_all_music_or_a_playlist_records_where_the_queue_came_from() {
    let list = library(&["a - 一", "a - 二"]);
    let ids = ids(&list);
    let mut player = Player::new(list);
    assert_eq!(player.queue_source(), None);
    assert_eq!(
        player.play_playlist(None, Some(&ids[1])).unwrap(),
        Step::Play(ids[1].clone())
    );
    assert_eq!(player.queue_source(), Some(ALL_MUSIC_SOURCE));

    // 播不了的情况什么都不变
    let empty = player.create_playlist("空").unwrap();
    assert_eq!(player.play_playlist(Some(&empty), None), Err(PlaylistError::Empty));
    assert_eq!(
        player.play_playlist(None, Some(&TrackId("a - 不存在".into()))),
        Err(PlaylistError::NotInList)
    );
    assert_eq!(
        player.play_playlist(Some(&PlaylistId("不存在".into())), None),
        Err(PlaylistError::NotFound)
    );
    assert_eq!(player.queue(), ids);
    assert_eq!(player.queue_source(), Some(ALL_MUSIC_SOURCE));

    // 从搜索结果这类临时列表点播后，播放列表不再对应任何歌单
    player.play_from(std::slice::from_ref(&ids[0]), &ids[0]).unwrap();
    assert_eq!(player.queue_source(), None);
}
