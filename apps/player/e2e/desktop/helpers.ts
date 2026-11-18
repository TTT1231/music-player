import { spawn, type ChildProcess } from "node:child_process";
import { cpSync, existsSync, readdirSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { browser } from "@wdio/globals";

const here = path.dirname(fileURLToPath(import.meta.url));
/** 被测应用：tauri dev/build 的调试产物 */
export const shellExe = path.resolve(here, "../../../../target/debug/music-player-shell.exe");
/** 测试素材：脚本生成的 fixtures */
export const fixturesDir = path.resolve(here, "../../../../fixtures/music");
/** 运行级临时数据目录（MUSIC_DATA_DIR），经 tauri-driver 的环境继承给被测应用 */
export const dataDir = path.resolve(here, "../../../../target/e2e-data");
export const inputDir = path.join(dataDir, "import-input");
const webviewDir = path.resolve(dataDir, "../e2e-webview");

export function stageFixtures(): string {
    cpSync(fixturesDir, inputDir, { recursive: true });
    return inputDir;
}

export const driverPort = 4444;
export const nativeDriverPort = 9515;

export function startDrivers(nativeDriverPath: string): ChildProcess {
    // msedgedriver 由 tauri-driver 在 Windows 上自动拉起；数据目录随环境继承给被测应用
    return spawn(
        process.env.TAURI_DRIVER_PATH ?? "tauri-driver",
        ["--port", String(driverPort), "--native-port", String(nativeDriverPort), "--native-driver", nativeDriverPath],
        {
            stdio: "inherit",
            env: {
                ...process.env,
                MUSIC_DATA_DIR: dataDir,
                WEBVIEW2_USER_DATA_FOLDER: webviewDir,
                WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: "",
            },
        },
    );
}

/** 清空数据目录：配合 browser.reloadSession() 让每个用例都从空目录启动 */
export function clearDataDir(): void {
    if (!existsSync(dataDir)) return;
    for (const entry of readdirSync(dataDir)) {
        rmSync(path.join(dataDir, entry), { recursive: true, force: true });
    }
}

/** 等待应用窗口就绪（主布局渲染完成） */
export async function waitForApp(): Promise<void> {
    await browser.waitUntil(
        () =>
            browser.execute(() => location.origin === "http://tauri.localhost" && !!document.querySelector(".desktop")),
        { timeout: 30_000, timeoutMsg: "应用窗口未就绪" },
    );
}

/** 通过应用自身的导入命令导入 fixtures 文件夹 */
export async function importFixtures(): Promise<{ added: string[]; withLyrics: number }> {
    const result = (await browser.execute(
        (dir: string) =>
            (
                window as unknown as {
                    __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
                }
            ).__TAURI_INTERNALS__.invoke("import_paths", { paths: [dir] }),
        stageFixtures(),
    )) as { added: string[]; withLyrics: number };
    await browser.waitUntil(async () => (await queueRows()).length === result.added.length, {
        timeout: 15_000,
        timeoutMsg: "导入后队列未刷新",
    });
    return result;
}

/** 当前队列里的歌名顺序 */
export function queueRows(): Promise<string[]> {
    return browser.execute(() =>
        [...document.querySelectorAll(".q-row")].map(row => row.querySelector(".t")?.textContent ?? ""),
    ) as Promise<string[]>;
}

/** 主舞台大标题（当前歌名） */
export async function currentTitle(): Promise<string> {
    return (await browser.execute(
        () => document.querySelector(".desktop-heading-text h1")?.textContent ?? "",
    )) as string;
}
