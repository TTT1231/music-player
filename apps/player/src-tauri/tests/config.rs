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
    assert_eq!(capability["permissions"], serde_json::json!(["core:default"]));
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
