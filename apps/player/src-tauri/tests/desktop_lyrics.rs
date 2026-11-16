use tauri::Manager;

#[test]
fn toggle_creates_the_lyrics_window() {
    let app = tauri::test::mock_builder().build(tauri::generate_context!()).unwrap();
    let handle = app.handle().clone();
    assert!(handle.get_webview_window("lyrics").is_none(), "初始状态没有歌词窗口");
    assert!(
        music_player_shell_lib::desktop_lyrics::toggle(&handle).unwrap(),
        "第一次打开返回 true"
    );
    assert!(
        handle.get_webview_window("lyrics").is_some(),
        "打开后应存在 lyrics 窗口"
    );
    // mock runtime 的窗口 getter 全是桩（title 返回空、close 只投递消息且仅在 run 循环处理），
    // 透明、置顶、无边框、跳过任务栏、关闭与重建路径只能由阶段 7 的桌面 e2e 在真实运行时覆盖。
}
