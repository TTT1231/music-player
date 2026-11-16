import assert from "node:assert/strict";
import { test } from "node:test";

import { androidReleaseManifestRemovesInternet, capabilityViolation, networkSourceViolation } from "./check-build.js";

test("networkSourceViolation 放行本地协议源，拦截外部网络源", () => {
    assert.equal(networkSourceViolation("'self'"), null);
    assert.equal(networkSourceViolation("asset:"), null);
    assert.equal(networkSourceViolation("http://asset.localhost"), null);
    assert.equal(networkSourceViolation("http://cover.localhost"), null);
    assert.equal(networkSourceViolation("http://ipc.localhost"), null);
    assert.equal(networkSourceViolation("http://evil.example.com"), "http://evil.example.com");
    assert.equal(networkSourceViolation("https://example.com"), "https://example.com");
    assert.equal(networkSourceViolation("http://asset.localhost.evil.com"), "http://asset.localhost.evil.com");
});

test("capabilityViolation 拦截协议类权限，放行本地 API 权限", () => {
    assert.equal(capabilityViolation("core:default"), null);
    assert.equal(capabilityViolation("core:window:allow-minimize"), null);
    assert.equal(capabilityViolation("core:event:default"), null);
    assert.equal(capabilityViolation("core:webview:allow-internal-toggle-devtools"), null);
    assert.equal(capabilityViolation("core:http:default"), "core:http:default");
    assert.equal(capabilityViolation("http:default"), "http:default");
    assert.equal(capabilityViolation("core:webview:allow-connect-remote"), "core:webview:allow-connect-remote");
    assert.equal(capabilityViolation("some-plugin:fetch-data"), "some-plugin:fetch-data");
});

test("androidReleaseManifestRemovesInternet 要求 release 清单以 tools:node=remove 声明 INTERNET", () => {
    assert.equal(
        androidReleaseManifestRemovesInternet(
            `<manifest xmlns:tools="http://schemas.android.com/tools">
                 <uses-permission android:name="android.permission.INTERNET" tools:node="remove" />
             </manifest>`,
        ),
        true,
    );
    assert.equal(androidReleaseManifestRemovesInternet("<manifest></manifest>"), false);
    assert.equal(
        androidReleaseManifestRemovesInternet(`<uses-permission android:name="android.permission.INTERNET" />`),
        false,
        "普通声明不是移除指令",
    );
});
