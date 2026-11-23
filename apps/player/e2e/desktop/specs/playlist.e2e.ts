import { expect } from "@wdio/globals";
import { readFileSync } from "node:fs";
import path from "node:path";
import { dataDir, importFixtures, queueRows, waitForApp } from "../helpers";

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

    it("右键删除当前自建歌单，全部音乐和播放队列保留，重启后歌单仍已删除", async () => {
        const savedPlaylists = () => {
            const session = JSON.parse(readFileSync(path.join(dataDir, "session.json"), "utf8")) as {
                saved_playlists: unknown[];
            };
            return session.saved_playlists.length;
        };
        await importFixtures();
        await (await browser.$('[aria-label="展开歌单"]')).click();
        await (await browser.$('[aria-label="新建歌单"]')).click();
        await (await browser.$("#playlist-name")).setValue("待删除");
        await (await browser.$("button=创建")).click();
        await browser.waitUntil(async () => (await browser.$("h1=待删除")).isExisting());
        await (await browser.$("button=添加歌曲")).click();
        await (await browser.$('[aria-label="选择 Track Two"]')).click();
        await (await browser.$("button=添加")).click();
        await (await browser.$(".playlist-picker")).waitForDisplayed({ reverse: true });
        await browser.waitUntil(() => savedPlaylists() === 1);

        const playlists = await browser.$('[aria-label="已保存的歌单"]');
        const allMusic = await playlists.$("button*=全部音乐");
        await allMusic.click({ button: "right" });
        expect(await (await browser.$(".playlist-context-menu")).isExisting()).toBe(false);
        const custom = await playlists.$("button*=待删除");
        await custom.click({ button: "right" });
        const deletion = await browser.$('.playlist-context-menu [role="menuitem"]');
        await expect(deletion).toHaveText("删除歌单「待删除」");
        await deletion.click();
        const confirmation = await browser.$('dialog[role="alertdialog"]');
        await (await confirmation.$("button=删除")).click();
        await (await browser.$("h1=全部音乐")).waitForDisplayed();
        expect(await custom.isExisting()).toBe(false);
        expect(await (await browser.$('[aria-label="播放 Track Two"]')).isExisting()).toBe(true);
        expect(await queueRows()).toHaveLength(4);

        await browser.waitUntil(() => savedPlaylists() === 0);
        await browser.reloadSession();
        await waitForApp();
        await (await browser.$('[aria-label="展开歌单"]')).click();
        expect(await (await browser.$("button*=待删除")).isExisting()).toBe(false);
        expect(await queueRows()).toHaveLength(4);
    });
});
