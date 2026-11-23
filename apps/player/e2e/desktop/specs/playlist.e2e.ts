import { expect } from "@wdio/globals";
import { importFixtures, queueRows, waitForApp } from "../helpers";

describe("桌面歌单", () => {
    it("创建歌单、从曲库添加歌曲并在重启后恢复", async () => {
        await importFixtures();
        await (await browser.$('[aria-label="展开歌单"]')).click();
        await (await browser.$('[aria-label="新建歌单"]')).click();
        await (await browser.$("#playlist-name")).setValue("我的收藏");
        await (await browser.$("button=创建")).click();
        await browser.waitUntil(async () => (await browser.$("h1=我的收藏")).isExisting());
        await (await browser.$("button=添加歌曲")).click();
        await (await browser.$('[aria-label="选择 Track Two"]')).click();
        await (await browser.$("button=添加")).click();
        await browser.waitUntil(async () => (await browser.$('[aria-label="从当前歌单移出 Track Two"]')).isExisting());
        expect(await queueRows()).toHaveLength(4);
        await browser.pause(1500);
        await browser.reloadSession();
        await waitForApp();
        await (await browser.$('[aria-label="展开歌单"]')).click();
        await browser.waitUntil(async () => (await browser.$("button*=我的收藏")).isExisting());
        await (await browser.$("button*=我的收藏")).click();
        expect(await (await browser.$('[aria-label="播放 Track Two"]')).isExisting()).toBe(true);
        expect(await queueRows()).toHaveLength(4);
    });
});
