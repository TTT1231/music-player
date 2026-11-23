import { expect } from "@wdio/globals";
import { importFixtures } from "../helpers";

describe("桌面歌单弹层与滚动区域", () => {
    it("新建歌单输入框使用深色样式并保持可见的焦点", async () => {
        await (await browser.$('[aria-label="展开歌单"]')).click();
        await (await browser.$('[aria-label="新建歌单"]')).click();
        const field = await browser.execute(() => {
            const input = document.querySelector<HTMLInputElement>("#playlist-name")!;
            const style = getComputedStyle(input);
            return {
                background: style.backgroundColor,
                height: input.getBoundingClientRect().height,
                focused: document.activeElement === input,
            };
        });
        expect(field.background).not.toBe("rgb(255, 255, 255)");
        expect(field.height).toBeGreaterThanOrEqual(32);
        expect(field.focused).toBe(true);
    });

    it("选歌弹层采用较亮的中灰表面", async () => {
        await importFixtures();
        await (await browser.$('[aria-label="展开歌单"]')).click();
        await (await browser.$('[aria-label="新建歌单"]')).click();
        await (await browser.$("#playlist-name")).setValue("收藏");
        await (await browser.$("button=创建")).click();
        await browser.waitUntil(async () => (await browser.$("h1=收藏")).isExisting());
        await (await browser.$("button=添加歌曲")).click();
        const background = await browser.execute(
            () => getComputedStyle(document.querySelector(".playlist-picker")!).backgroundColor,
        );
        const channels = background.match(/\d+/g)!.slice(0, 3).map(Number);
        expect(Math.min(...channels)).toBeGreaterThanOrEqual(60);
    });

    it("本次选择显示蓝色，已经添加的歌曲保持不可选择", async () => {
        const imported = await importFixtures();
        await (await browser.$('[aria-label="展开歌单"]')).click();
        await (await browser.$('[aria-label="新建歌单"]')).click();
        await (await browser.$("#playlist-name")).setValue("收藏");
        await (await browser.$("button=创建")).click();
        await browser.waitUntil(async () => (await browser.$("h1=收藏")).isExisting());
        await browser.execute(
            (track: string) =>
                (
                    window as unknown as {
                        __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
                    }
                ).__TAURI_INTERNALS__.invoke("add_playlist_tracks", { id: "list-1", tracks: [track] }),
            imported.added[1],
        );
        await browser.waitUntil(async () => (await browser.$('[aria-label="从当前歌单移出 Track Two"]')).isExisting());
        await (await browser.$("button=添加歌曲")).click();
        await (await browser.$('[aria-label="选择 Track Three"]')).click();
        const state = await browser.execute(() => {
            const added = document.querySelector<HTMLInputElement>('[aria-label="选择 Track Two"]')!;
            const selected = document.querySelector<HTMLInputElement>('[aria-label="选择 Track Three"]')!;
            const all = document.querySelector<HTMLInputElement>(".playlist-select-all input")!;
            return {
                addedDisabled: added.disabled,
                addedChecked: added.checked,
                selectedChecked: selected.checked,
                selectedAccent: getComputedStyle(selected).accentColor,
                allAccent: getComputedStyle(all).accentColor,
            };
        });
        const channels = state.selectedAccent.match(/\d+/g)?.slice(0, 3).map(Number) ?? [];
        expect(state.addedDisabled && state.addedChecked && state.selectedChecked).toBe(true);
        expect(channels).toHaveLength(3);
        expect(channels[2]).toBeGreaterThan(channels[0]);
        expect(channels[2]).toBeGreaterThan(channels[1]);
        expect(state.allAccent).toBe(state.selectedAccent);
    });

    it("滚动区域与播放进度条的操作区留出间距", async () => {
        await importFixtures();
        await (await browser.$('[aria-label="展开歌单"]')).click();
        const metrics = await browser.execute(() => {
            const queue = document.querySelector(".q-list")!;
            const library = document.querySelector(".playlist-table-wrap")!;
            const progress = document.querySelector(".desktop-progress")!.getBoundingClientRect();
            return {
                queueGap: progress.top - queue.getBoundingClientRect().bottom,
                libraryGap: progress.top - library.getBoundingClientRect().bottom,
                scrollbarButtons: getComputedStyle(library, "::-webkit-scrollbar-button").display,
            };
        });
        expect(metrics.queueGap).toBeGreaterThanOrEqual(8);
        expect(metrics.libraryGap).toBeGreaterThanOrEqual(8);
        expect(metrics.scrollbarButtons).toBe("none");
    });
});
