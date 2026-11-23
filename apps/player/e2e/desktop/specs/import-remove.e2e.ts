import { expect } from "@wdio/globals";
import { existsSync } from "node:fs";
import path from "node:path";
import { currentTitle, dataDir, importFixtures, inputDir, queueRows } from "../helpers";

/** 通过应用命令播放一首歌（需要可用音频输出设备） */
function playTrack(id: string): void {
    browser.execute(
        (trackId: string) =>
            (
                window as unknown as {
                    __TAURI_INTERNALS__: { invoke: (cmd: string, args?: unknown) => Promise<unknown> };
                }
            ).__TAURI_INTERNALS__.invoke("play_track", { id: trackId }),
        id,
    );
}

describe("从队列和曲库移除", () => {
    it("队列移除保留文件，曲库确认移除后删除对应的音乐和歌词", async () => {
        await importFixtures();
        const before = await queueRows();
        expect(before).toHaveLength(4);

        const index = before.indexOf("Track Two");
        const row = (await browser.$$(".q-row"))[index];
        const audio = path.join(dataDir, "assets", "music", "Test Artist - Track Two.ogg");
        const lyrics = path.join(dataDir, "assets", "lysis", "Test Artist - Track Two.lrc");
        expect(existsSync(audio)).toBe(true);
        expect(existsSync(lyrics)).toBe(true);
        expect(existsSync(path.join(inputDir, "Test Artist - Track Two.ogg"))).toBe(false);
        expect(existsSync(path.join(inputDir, "Test Artist - Track Two.lrc"))).toBe(false);
        await row.click();
        await browser.keys(["Delete"]);
        await browser.waitUntil(async () => (await queueRows()).length === 3);
        expect(existsSync(audio)).toBe(true);
        expect(existsSync(lyrics)).toBe(true);
        await (await browser.$('[aria-label="展开歌单"]')).click();
        const remove = await browser.$('[aria-label="从曲库移除 Track Two"]');
        await remove.click();
        await (await browser.$("button=取消")).click();
        expect(existsSync(audio)).toBe(true);
        await remove.click();
        await (await browser.$("button=确认移除")).click();
        await browser.waitUntil(() => !existsSync(audio));

        expect(await queueRows()).not.toContain("Track Two");
        expect(existsSync(audio)).toBe(false);
        expect(existsSync(lyrics)).toBe(false);
        expect(existsSync(path.join(dataDir, "assets", "music", "No Artist Instrumental.mp3"))).toBe(true);
    });

    it("移除正在播放的歌曲时接着播放列表中的下一首", async () => {
        await importFixtures();
        // 播放第一首（需要可用音频输出设备），然后把它移除
        // BiDi 元素的 getAttribute 返回的是包装对象，取 id 必须在页面内读 dataset
        playTrack(await browser.execute(() => (document.querySelector(".q-row") as HTMLElement).dataset.id!));
        await browser.waitUntil(
            async () =>
                (await browser.execute(() =>
                    document.querySelector(".desktop-control-play")?.getAttribute("aria-label"),
                )) === "暂停",
            { timeoutMsg: "第一首未进入播放状态" },
        );
        // 播放状态确认后再抓歌名，避免拿到空状态文案
        const playing = await currentTitle();
        expect(playing).not.toBe("选一首歌开始");
        const rows = await queueRows();
        const index = rows.indexOf(playing);
        const row = (await browser.$$(".q-row"))[index];
        await row.click();
        await browser.keys(["Delete"]);
        await browser.waitUntil(async () => (await queueRows()).length === 3);

        const remaining = await queueRows();
        expect(await currentTitle()).toBe(remaining[Math.min(index, remaining.length - 1)]);
        expect(
            await browser.execute(() => document.querySelector(".desktop-control-play")?.getAttribute("aria-label")),
        ).toBe("暂停");
    });
});
