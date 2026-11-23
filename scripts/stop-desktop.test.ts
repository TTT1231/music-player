import assert from "node:assert/strict";
import { test } from "node:test";

import {
    DEFAULT_DEV_PORT,
    devPortFromConfig,
    listeningPidsFromLsof,
    listeningPidsFromNetstat,
} from "./stop-desktop.js";

test("devPortFromConfig 从 beforeDevCommand 读端口，读不到时回退默认值", () => {
    assert.equal(
        devPortFromConfig({ build: { beforeDevCommand: "pnpm exec vite --host 127.0.0.1 --port 1420 --strictPort" } }),
        1420,
    );
    assert.equal(devPortFromConfig({ build: { beforeDevCommand: "pnpm exec vite --port 3000" } }), 3000);
    assert.equal(devPortFromConfig({ build: {} }), DEFAULT_DEV_PORT);
    assert.equal(devPortFromConfig({}), DEFAULT_DEV_PORT);
});

test("listeningPidsFromNetstat 只收集监听目标端口的进程，兼容 IPv6 与缩进", () => {
    const output = [
        "",
        "活动连接",
        "",
        "  协议  本地地址          外部地址        状态           PID",
        "  TCP    127.0.0.1:1420         0.0.0.0:0              LISTENING       4321",
        "  TCP    127.0.0.1:1420         127.0.0.1:52000        ESTABLISHED     4321",
        "  TCP    [::1]:1420             [::]:0                 LISTENING       8765",
        "  TCP    127.0.0.1:1421         0.0.0.0:0              LISTENING       1111",
        "  TCP    0.0.0.0:1420           0.0.0.0:0              LISTENING       4321",
        "",
    ].join("\r\n");
    assert.deepEqual(listeningPidsFromNetstat(output, 1420).sort(), [4321, 8765]);
    assert.deepEqual(listeningPidsFromNetstat(output, 9999), []);
});

test("listeningPidsFromLsof 解析 PID 列表并去重", () => {
    assert.deepEqual(listeningPidsFromLsof("4321\n"), [4321]);
    assert.deepEqual(listeningPidsFromLsof("4321\n8765\n4321\n"), [4321, 8765]);
    assert.deepEqual(listeningPidsFromLsof(""), []);
});
