import { expect } from "@wdio/globals";
import { copyFileSync, mkdirSync, rmSync } from "node:fs";
import path from "node:path";
import { dataDir, fixturesDir, queueRows, stageFixtures } from "../helpers";

/**
 * 发出与系统拖放相同的 Tauri 事件。WebDriver 无法从资源管理器真正拖文件，
 * 但从这里开始走的是应用真实的拖放监听、导入对话框和导入命令。
 */
async function dropFiles(paths: string[]): Promise<void> {
    await browser.execute(async (files: string[]) => {
        const { invoke } = (
            window as unknown as {
                __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
            }
        ).__TAURI_INTERNALS__;
        const target = { kind: "Webview", label: "main" };
        const position = { x: 400, y: 300 };
        await invoke("plugin:event|emit_to", {
            target,
            event: "tauri://drag-enter",
            payload: { paths: files, position },
        });
        await invoke("plugin:event|emit_to", {
            target,
            event: "tauri://drag-drop",
            payload: { paths: files, position },
        });
    }, paths);
}

async function importText(): Promise<string> {
    await browser.waitUntil(
        async () => Boolean(await browser.execute(() => document.querySelector(".import-result"))),
        {
            timeout: 15_000,
            timeoutMsg: "拖放后没有导入结果",
        },
    );
    return (await browser.execute(() => document.querySelector(".import-result p")?.textContent ?? "")) as string;
}

/** 某首歌的行是否完整显示在队列可视区域内 */
function rowVisible(title: string): Promise<boolean> {
    return browser.execute((name: string) => {
        const list = document.querySelector(".q-list")!.getBoundingClientRect();
        const row = [...document.querySelectorAll(".q-row")].find(
            item => item.querySelector(".t")?.textContent === name,
        );
        if (!row) return false;
        const rect = row.getBoundingClientRect();
        // 允许 1px：缩放后滚动位置可能停在亚像素上
        return rect.top >= list.top - 1 && rect.bottom <= list.bottom + 1;
    }, title) as Promise<boolean>;
}

describe("拖放导入与去听歌", () => {
    it("把歌曲文件拖进窗口即可导入", async () => {
        const inputs = stageFixtures();
        const files = [
            "Test Artist - Track Two.ogg",
            "Test Artist - Track Two.lrc",
            "Test Artist、Guest - Track One (Live).mp3",
        ].map(name => path.join(inputs, name));
        await dropFiles(files);

        expect(await browser.execute(() => Boolean(document.querySelector(".import-dialog")))).toBe(true);
        expect(await importText()).toContain("已导入 2 首");
        expect(await queueRows()).toEqual(["Track Two", "Track One (Live)"]);
    });

    it("点“去听歌”关闭导入界面，列表定位到第一首导入的歌曲", async () => {
        // 先放 20 首占满队列，再拖入一首新歌：它排在末尾、初始不在可视区域内
        const fillers = path.join(dataDir, "fillers");
        rmSync(fillers, { recursive: true, force: true });
        mkdirSync(fillers, { recursive: true });
        const source = path.join(fixturesDir, "No Artist Instrumental.mp3");
        const paths = Array.from({ length: 20 }, (_, index) => {
            const file = path.join(fillers, `Filler ${String(index + 1).padStart(2, "0")}.mp3`);
            copyFileSync(source, file);
            return file;
        });
        await browser.execute(
            (files: string[]) =>
                (
                    window as unknown as {
                        __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
                    }
                ).__TAURI_INTERNALS__.invoke("import_paths", { paths: files }),
            paths,
        );
        await browser.waitUntil(async () => (await queueRows()).length === 20, { timeoutMsg: "占位歌曲未导入" });

        await dropFiles([path.join(stageFixtures(), "Test Artist - Track Two.ogg")]);
        expect(await importText()).toContain("已导入 1 首");
        expect(await rowVisible("Track Two")).toBe(false);

        const goListen = await browser.$("button=去听歌");
        await goListen.click();
        await browser.waitUntil(async () => !(await browser.execute(() => document.querySelector(".import-dialog"))), {
            timeoutMsg: "去听歌后导入界面未关闭",
        });
        await browser.waitUntil(() => rowVisible("Track Two"), {
            timeout: 5_000,
            timeoutMsg: "列表没有定位到导入的歌曲",
        });
    });
});
