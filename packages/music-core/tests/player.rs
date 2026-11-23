//! 验收：播放。五种播放模式、上一首/下一首、按列表当前顺序切歌、移除正在播放的歌、
//! 无法读取时跳到下一首。

mod common;

use std::time::Duration;

use common::{ids, library};
use music_core::{PITCH_RANGE, PlayMode, Player, RATE_RANGE, RESTART_THRESHOLD, Step, TrackId};

/// 四首歌的播放器，正在播放第 `at` 首（从 0 开始）。
fn playing_at(mode: PlayMode, at: usize) -> (Player, Vec<TrackId>) {
    let list = library(&["a - 一", "a - 二", "a - 三", "a - 四"]);
    let ids = ids(&list);
    let mut p = Player::new(list);
    p.set_mode(mode);
    p.play_track(&ids[at]);
    (p, ids)
}

#[test]
fn starts_paused_with_defaults() {
    let p = Player::new(library(&["a - 一"]));
    assert_eq!(p.current(), None);
    assert!(!p.is_playing());
    assert_eq!(p.mode(), PlayMode::Loop);
    assert_eq!(p.volume(), 0.8);
    assert_eq!(p.rate(), 1.0);
    assert_eq!(p.pitch(), 0);
}

#[test]
fn play_pause() {
    let (mut p, ids) = playing_at(PlayMode::Loop, 1);
    assert_eq!(p.current(), Some(&ids[1]));
    assert!(p.is_playing());
    p.pause();
    assert!(!p.is_playing());
    p.play();
    assert!(p.is_playing());
}

#[test]
fn next_follows_list_order_and_wraps() {
    let (mut p, ids) = playing_at(PlayMode::Order, 2);
    assert_eq!(p.next(), Some(Step::Play(ids[3].clone())));
    assert_eq!(p.next(), Some(Step::Play(ids[0].clone())));
    assert_eq!(p.current(), Some(&ids[0]));
}

#[test]
fn next_changes_track_even_in_single_track_modes() {
    for mode in [PlayMode::RepeatOne, PlayMode::Single] {
        let (mut p, ids) = playing_at(mode, 0);
        assert_eq!(p.next(), Some(Step::Play(ids[1].clone())), "{mode:?}");
    }
}

#[test]
fn prev_restarts_after_threshold_otherwise_goes_back_and_wraps() {
    let (mut p, ids) = playing_at(PlayMode::Loop, 0);
    assert_eq!(
        p.prev(RESTART_THRESHOLD + Duration::from_millis(1)),
        Some(Step::Restart)
    );
    assert_eq!(p.current(), Some(&ids[0]));
    assert_eq!(p.prev(Duration::from_secs(1)), Some(Step::Play(ids[3].clone())));
}

#[test]
fn next_and_prev_on_empty_list_do_nothing() {
    let mut p = Player::new(library(&[]));
    assert_eq!(p.next(), None);
    assert_eq!(p.prev(Duration::ZERO), None);
}

#[test]
fn order_mode_stops_after_last_track() {
    let (mut p, ids) = playing_at(PlayMode::Order, 2);
    assert_eq!(p.on_ended(), Step::Play(ids[3].clone()));
    assert_eq!(p.on_ended(), Step::Stop);
    assert_eq!(p.current(), Some(&ids[3]));
    assert!(!p.is_playing());
}

#[test]
fn loop_mode_continues_from_first_after_last() {
    let (mut p, ids) = playing_at(PlayMode::Loop, 3);
    assert_eq!(p.on_ended(), Step::Play(ids[0].clone()));
    assert!(p.is_playing());
}

#[test]
fn shuffle_never_picks_current_track_twice_in_a_row() {
    let (mut p, _) = playing_at(PlayMode::Shuffle, 0);
    let mut seen = std::collections::HashSet::new();
    for i in 0..200 {
        let before = p.current().cloned().unwrap();
        // 自然播完和点“下一首”都要满足
        let step = if i % 2 == 0 { p.on_ended() } else { p.next().unwrap() };
        let Step::Play(id) = step else {
            panic!("随机播放应继续播放：{step:?}")
        };
        assert_ne!(id, before);
        seen.insert(id);
    }
    assert_eq!(seen.len(), 4, "随机播放应覆盖所有歌曲");
}

#[test]
fn shuffle_with_one_track_replays_it() {
    let list = library(&["a - 唯一"]);
    let id = ids(&list)[0].clone();
    let mut p = Player::new(list);
    p.set_mode(PlayMode::Shuffle);
    p.play_track(&id);
    assert_eq!(p.on_ended(), Step::Play(id));
}

#[test]
fn repeat_one_replays_current_track() {
    let (mut p, ids) = playing_at(PlayMode::RepeatOne, 1);
    assert_eq!(p.on_ended(), Step::Restart);
    assert_eq!(p.current(), Some(&ids[1]));
    assert!(p.is_playing());
}

#[test]
fn single_mode_stops_after_current_track() {
    let (mut p, ids) = playing_at(PlayMode::Single, 1);
    assert_eq!(p.on_ended(), Step::Stop);
    assert_eq!(p.current(), Some(&ids[1]));
    assert!(!p.is_playing());
}

#[test]
fn next_and_prev_follow_reordered_list() {
    let (mut p, ids) = playing_at(PlayMode::Loop, 0);
    // 一 二 三 四 → 二 三 一 四
    p.move_track(0, 2);
    assert_eq!(p.next(), Some(Step::Play(ids[3].clone())));
    p.play_track(&ids[0]);
    assert_eq!(p.prev(Duration::ZERO), Some(Step::Play(ids[2].clone())));
}

#[test]
fn removing_playing_track_continues_with_the_following_one() {
    let (mut p, ids) = playing_at(PlayMode::Loop, 1);
    assert_eq!(p.remove(&ids[1]), Some(Step::Play(ids[2].clone())));
    assert_eq!(p.current(), Some(&ids[2]));
    assert!(p.is_playing());
    assert_eq!(p.queue().len(), 3);
}

#[test]
fn move_track_changes_queue_order() {
    let list = library(&["a - 一", "a - 二", "a - 三", "a - 四"]);
    let ids = ids(&list);
    let mut p = Player::new(list);
    p.move_track(0, 2);
    assert_eq!(
        p.queue(),
        [ids[1].clone(), ids[2].clone(), ids[0].clone(), ids[3].clone()]
    );
    p.move_track(3, 0);
    assert_eq!(
        p.queue(),
        [ids[3].clone(), ids[1].clone(), ids[2].clone(), ids[0].clone()]
    );
    // 只调整播放列表，曲库仍是导入顺序
    assert_eq!(common::ids(p.library()), ids);
}

#[test]
fn removing_last_playing_track_wraps_to_first() {
    let (mut p, ids) = playing_at(PlayMode::Loop, 3);
    assert_eq!(p.remove(&ids[3]), Some(Step::Play(ids[0].clone())));
}

#[test]
fn removing_other_track_keeps_current() {
    let (mut p, ids) = playing_at(PlayMode::Loop, 1);
    assert_eq!(p.remove(&ids[3]), None);
    assert_eq!(p.current(), Some(&ids[1]));
}

#[test]
fn removing_only_track_stops() {
    let list = library(&["a - 唯一"]);
    let id = ids(&list)[0].clone();
    let mut p = Player::new(list);
    p.play_track(&id);
    assert_eq!(p.remove(&id), Some(Step::Stop));
    assert_eq!(p.current(), None);
    assert!(!p.is_playing());
}

#[test]
fn unreadable_track_skips_to_next_in_list_order() {
    for mode in [PlayMode::Order, PlayMode::RepeatOne, PlayMode::Single] {
        let (mut p, ids) = playing_at(mode, 3);
        assert_eq!(p.on_unreadable(), Step::Play(ids[0].clone()), "{mode:?}");
    }
}

#[test]
fn unreadable_only_track_stops() {
    let list = library(&["a - 唯一"]);
    let id = ids(&list)[0].clone();
    let mut p = Player::new(list);
    p.play_track(&id);
    assert_eq!(p.on_unreadable(), Step::Stop);
    assert!(!p.is_playing());
}

#[test]
fn rate_is_clamped_and_rounded_to_one_decimal() {
    let mut p = Player::new(library(&[]));
    assert_eq!(p.set_rate(1.24), 1.2);
    assert_eq!(p.set_rate(9.0), *RATE_RANGE.end());
    assert_eq!(p.set_rate(0.1), *RATE_RANGE.start());
    assert_eq!(p.rate(), 0.5);
}

#[test]
fn pitch_and_volume_are_clamped() {
    let mut p = Player::new(library(&[]));
    // 升降调 ±5 档，与倍速 0.5–1.5 每侧 5 档对应
    assert_eq!(PITCH_RANGE, -5..=5);
    assert_eq!(p.set_pitch(12), 5);
    assert_eq!(p.set_pitch(-12), -5);
    p.set_volume(1.5);
    assert_eq!(p.volume(), 1.0);
    p.set_volume(-1.0);
    assert_eq!(p.volume(), 0.0);
}
