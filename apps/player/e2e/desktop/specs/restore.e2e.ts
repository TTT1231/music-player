import { expect } from "@wdio/globals";
import { browser } from "@wdio/globals";
import { currentTitle, importFixtures, queueRows, waitForApp } from "../helpers";

describe("重启后恢复", () => {
    it("恢复播放列表及顺序、上次的歌曲（暂停状态）、播放模式和音量", async () => {
        await importFixtures();
        const order = await queueRows();

        // 播放第一首再暂停：恢复要求“上次的歌曲（暂停状态）”
        await browser.execute(() =>
            (
                window as unknown as {
                    __TAURI_INTERNALS__: { invoke: (cmd: string, args?: unknown) => Promise<unknown> };
                }
            ).__TAURI_INTERNALS__.invoke("play_track", {
                id: (document.querySelector(".q-row") as HTMLElement).dataset.id,
            }),
        );
        await browser.waitUntil(async () => (await currentTitle()) === order[0], { timeoutMsg: "当前歌曲未切换" });
        await browser.execute(() =>
            (
                window as unknown as {
                    __TAURI_INTERNALS__: { invoke: (cmd: string, args?: unknown) => Promise<unknown> };
                }
            ).__TAURI_INTERNALS__.invoke("toggle"),
        );
        await browser.waitUntil(
            async () =>
                (await browser.execute(() =>
                    document.querySelector(".desktop-control-play")?.getAttribute("aria-label"),
                )) === "播放",
            { timeoutMsg: "未能暂停" },
        );
        await browser.execute(() =>
            (
                window as unknown as {
                    __TAURI_INTERNALS__: { invoke: (cmd: string, args?: unknown) => Promise<unknown> };
                }
            ).__TAURI_INTERNALS__.invoke("set_mode", { mode: "shuffle" }),
        );
        await browser.execute(() =>
            (
                window as unknown as {
                    __TAURI_INTERNALS__: { invoke: (cmd: string, args?: unknown) => Promise<unknown> };
                }
            ).__TAURI_INTERNALS__.invoke("set_volume", { volume: 0.4 }),
        );
        // 等防抖保存（约 0.5 秒）落盘后再重启
        await browser.pause(1500);

        await browser.reloadSession();
        await waitForApp();
        await browser.waitUntil(async () => (await queueRows()).length === order.length, {
            timeoutMsg: "重启后列表未恢复",
        });

        expect(await queueRows()).toEqual(order);
        expect(await currentTitle()).toBe(order[0]);
        expect(
            await browser.execute(() => document.querySelector(".desktop-control-play")?.getAttribute("aria-label")),
        ).toBe("播放");
        expect(
            await browser.execute(() =>
                document.querySelector('[aria-label^="播放模式："]')?.getAttribute("aria-label"),
            ),
        ).toBe("播放模式：随机播放");

        // 音量弹层显示 40%
        await browser.execute(() => (document.querySelector('[aria-label="音量"]') as HTMLElement).click());
        await browser.waitUntil(
            async () => await browser.execute(() => Boolean(document.querySelector(".desktop-volume-value"))),
        );
        expect(await browser.execute(() => document.querySelector(".desktop-volume-value")?.textContent)).toBe("40%");
    });
});
