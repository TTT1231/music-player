use serde_json::Value;

#[test]
fn app_configuration_keeps_runtime_local_and_targets_nsis() {
    let config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    assert_eq!(config["identifier"], "local.music.player");
    assert_eq!(config["bundle"]["targets"], "nsis");
    let csp = config["app"]["security"]["csp"].as_str().unwrap();
    assert!(csp.contains("default-src 'self'"));
    assert!(csp.contains("cover:"));
    for source in csp.split(|c: char| c.is_whitespace() || c == ';') {
        if source.starts_with("http:") || source.starts_with("https:") {
            assert!(
                [
                    "http://asset.localhost",
                    "http://cover.localhost",
                    "http://ipc.localhost"
                ]
                .contains(&source),
                "unexpected network source: {source}"
            );
        }
    }
    let capability: Value = serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
    assert!(capability.get("remote").is_none());
    assert_eq!(capability["windows"], serde_json::json!(["main"]));
    assert_eq!(
        capability["permissions"],
        serde_json::json!([
            "core:default",
            "playback:default",
            "core:window:allow-minimize",
            "core:window:allow-toggle-maximize",
            "core:window:allow-close",
            "core:window:allow-start-dragging"
        ])
    );
    assert_eq!(config["app"]["windows"][0]["decorations"], false);
    // 原型布局以 1024px 宽为下限（min-width: 1024px），窗口不能缩得更窄，否则右侧队列和底栏按钮被裁掉
    assert_eq!(config["app"]["windows"][0]["minWidth"], 1024);
    assert_eq!(config["app"]["windows"][0]["minHeight"], 640);
}

#[test]
fn lyrics_window_capability_is_local_and_minimal() {
    // 桌面歌词窗口独立于主窗口授权：只允许本地命令、窗口拖动和字号自适应的窗口尺寸调整
    let capability: Value = serde_json::from_str(include_str!("../capabilities/lyrics.json")).unwrap();
    assert!(capability.get("remote").is_none());
    assert_eq!(capability["windows"], serde_json::json!(["lyrics"]));
    assert_eq!(
        capability["permissions"],
        serde_json::json!([
            "core:default",
            "core:window:allow-set-size",
            "core:window:allow-start-dragging"
        ])
    );
}

#[test]
fn main_window_allows_frontend_drag_drop_listeners() {
    let config: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    assert_eq!(config["app"]["windows"][0]["dragDropEnabled"], true);
    let capability: Value = serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
    assert!(
        capability["windows"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("main"))
    );
    assert!(
        capability["permissions"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("core:default"))
    );

    let manifest: Value = serde_json::from_str(include_str!("../gen/schemas/acl-manifests.json")).unwrap();
    let core = manifest["core"]["default_permission"]["permissions"]
        .as_array()
        .unwrap();
    assert!(core.contains(&serde_json::json!("core:event:default")));
    let events = manifest["core:event"]["default_permission"]["permissions"]
        .as_array()
        .unwrap();
    assert!(events.contains(&serde_json::json!("allow-listen")));
    assert!(events.contains(&serde_json::json!("allow-unlisten")));
}
