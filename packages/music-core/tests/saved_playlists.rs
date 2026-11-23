mod common;

use common::{ids, playlist};
use music_core::{Player, TrackId};

#[test]
fn saved_lists_are_distinct_from_library_and_queue_and_survive_restore() {
    let list = playlist(&["a - 一", "a - 二", "a - 三"]);
    let ids = ids(&list);
    let mut player = Player::new(list);
    let list_id = player.create_saved_playlist("  喜欢  ").unwrap();
    assert!(player.create_saved_playlist("喜欢").is_err());
    assert!(player.create_saved_playlist("全部音乐").is_err());
    player
        .add_to_saved_playlist(
            &list_id,
            &[
                ids[1].clone(),
                ids[0].clone(),
                ids[1].clone(),
                TrackId("missing".into()),
            ],
        )
        .unwrap();
    player.play_saved_playlist(Some(&list_id), Some(&ids[1])).unwrap();
    assert_eq!(
        player.playlist().tracks().iter().map(|t| &t.id).collect::<Vec<_>>(),
        vec![&ids[1], &ids[0]]
    );
    player.remove_from_queue(&ids[1]);
    assert_eq!(player.library().len(), 3);
    assert_eq!(
        player.saved_playlists()[0].track_ids,
        vec![ids[1].clone(), ids[0].clone()]
    );
    let restored = Player::restore(player.session());
    assert_eq!(restored.library().len(), 3);
    assert_eq!(restored.playlist().len(), 1);
    assert_eq!(restored.saved_playlists()[0].track_ids.len(), 2);
}

#[test]
fn removing_library_track_prunes_saved_lists_but_removing_from_list_does_not_touch_queue() {
    let list = playlist(&["a - 一", "a - 二"]);
    let ids = ids(&list);
    let mut player = Player::new(list);
    let list_id = player.create_saved_playlist("歌单").unwrap();
    player.add_to_saved_playlist(&list_id, &ids).unwrap();
    player.remove_from_saved_playlist(&list_id, &ids[0]).unwrap();
    assert_eq!(player.playlist().len(), 2);
    player.remove(&ids[1]);
    assert!(player.saved_playlists()[0].track_ids.is_empty());
}

#[test]
fn queue_source_survives_edits_and_restart() {
    let mut player = Player::new(playlist(&["a - 一", "a - 二"]));
    let list_id = player.create_saved_playlist("收藏").unwrap();
    let ids = ids(player.library());
    player.add_to_saved_playlist(&list_id, &ids).unwrap();
    player.play_saved_playlist(Some(&list_id), None).unwrap();
    player.remove_from_queue(&ids[0]);
    assert_eq!(player.queue_source(), Some(list_id.as_str()));
    assert_eq!(Player::restore(player.session()).queue_source(), Some(list_id.as_str()));
}

#[test]
fn library_only_import_does_not_append_to_queue() {
    let mut player = Player::new(playlist(&["a - 一"]));
    player.import_library([common::file("b - 二.mp3")]);
    assert_eq!(player.library().len(), 2);
    assert_eq!(player.playlist().len(), 1);
}
