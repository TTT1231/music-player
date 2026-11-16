import { expect } from "@wdio/globals";
import { importFixtures } from "../helpers";

describe("桌面歌词窗口", () => {
    it("点词打开独立窗口，两行显示；关闭后回到单窗口", async () => {
        await importFixtures();

        await browser.execute(() => (document.querySelector(".desktop-lyrics-button") as HTMLElement).click());
        await browser.waitUntil(async () => (await browser.getWindowHandles()).length === 2, {
            timeout: 15_000,
            timeoutMsg: "桌面歌词窗口未打开",
        });

        const handles = await browser.getWindowHandles();
        const main = await browser.getWindowHandle();
        await browser.switchToWindow(handles.find(handle => handle !== main)!);
        await browser.waitUntil(async () => await browser.execute(() => Boolean(document.querySelector(".desk"))), {
            timeoutMsg: "歌词窗口未渲染",
        });
        const lines = (await browser.execute(() =>
            [...document.querySelectorAll(".dl .base")].map(node => node.textContent ?? ""),
        )) as string[];
        // 无当前歌曲时至少显示应用名一行；恢复会话后可能带出当前歌曲与前奏预告
        expect(lines.length).toBe(2);
        expect(lines.filter(text => text.length > 0).length).toBeGreaterThan(0);

        await browser.execute(() => (document.querySelector(".desk-bar .close") as HTMLElement).click());
        await browser.switchToWindow(main);
        await browser.waitUntil(async () => (await browser.getWindowHandles()).length === 1, {
            timeout: 10_000,
            timeoutMsg: "桌面歌词窗口未关闭",
        });
        expect(await browser.getWindowHandles()).toHaveLength(1);
    });
});
