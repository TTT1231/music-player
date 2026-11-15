use music_core::{LyricLine, Lyrics};
use music_player_shell_lib::ipc::lyrics_response;

#[test]
fn lyrics_payload_uses_the_public_contract() {
    let payload = lyrics_response(Lyrics {
        lines: vec![LyricLine {
            time_ms: 1234,
            text: "歌词".into(),
        }],
    });
    assert_eq!(
        serde_json::to_value(payload).unwrap(),
        serde_json::json!({
            "lines": [{"timeMs": 1234, "text": "歌词"}]
        })
    );
}
