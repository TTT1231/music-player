import type { ChildProcess } from "node:child_process";
import { mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { clearDataDir, dataDir, driverPort, shellExe, startDrivers } from "./helpers";

const here = path.dirname(fileURLToPath(import.meta.url));
/** 与本机 Edge/WebView2 版本匹配的 msedgedriver，见 docs/testing.md 的环境准备 */
const nativeDriverPath = process.env.MSEDGEDRIVER_PATH ?? path.resolve(here, "../../../../.tools/msedgedriver.exe");

let driver: ChildProcess | undefined;

export const config = {
    runner: "local",
    specs: [path.resolve(here, "specs/*.e2e.ts")],
    maxInstances: 1,
    hostname: "127.0.0.1",
    port: driverPort,
    capabilities: [
        {
            // tauri-driver 只认 tauri:options；browserName 是 WDIO 必填项
            browserName: "wry",
            "tauri:options": { application: shellExe },
        },
    ],
    framework: "mocha",
    mochaOpts: { timeout: 60_000, ui: "bdd" },
    reporters: ["spec"],
    onPrepare() {
        mkdirSync(dataDir, { recursive: true });
        driver = startDrivers(nativeDriverPath);
    },
    onComplete() {
        driver?.kill();
    },
    async beforeTest() {
        // 每个用例从空的数据目录启动：清空后重建会话（tauri-driver 会重新启动应用进程）
        clearDataDir();
        await browser.reloadSession();
        await browser.waitUntil(
            async () => await browser.execute(() => Boolean(document.querySelector(".desktop, .q-empty"))),
            { timeout: 30_000, timeoutMsg: "应用窗口未就绪" },
        );
    },
};
