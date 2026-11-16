import { expect } from "@wdio/globals";
import { importFixtures, queueRows } from "../helpers";

async function dragRowTo(row: WebdriverIO.Element, offsetY: number): Promise<void> {
    // BiDi 定位的元素没有 elementId，getElementRect 走不了经典命令；改用页面内测量
    const rect = (await browser.execute((el: HTMLElement) => {
        const r = el.getBoundingClientRect();
        return { x: r.x, y: r.y, width: r.width, height: r.height };
    }, row)) as { x: number; y: number; width: number; height: number };
    const startX = Math.round(rect.x + rect.width / 2);
    const startY = Math.round(rect.y + rect.height / 2);
    // 移动超过几像素（阈值 5px）才开始拖动，因此分两段移动
    await browser.performActions([
        {
            type: "pointer",
            id: "drag",
            parameters: { pointerType: "mouse" },
            actions: [
                { type: "pointerMove", duration: 0, x: startX, y: startY },
                { type: "pointerDown", button: 0 },
                { type: "pointerMove", duration: 120, x: startX, y: startY + 30 },
                { type: "pointerMove", duration: 150, x: startX, y: startY + offsetY },
                { type: "pause", duration: 120 },
                { type: "pointerUp", button: 0 },
            ],
        },
    ]);
    await browser.releaseActions();
}

describe("调整播放顺序", () => {
    it("按住行拖动可调整顺序", async () => {
        await importFixtures();
        const before = await queueRows();
        const rows = await browser.$$(".q-row");
        await dragRowTo(rows[0], 90);
        await browser.waitUntil(async () => (await queueRows()).join("|") !== before.join("|"), {
            timeoutMsg: "拖动后顺序未变化",
        });
        const after = await queueRows();
        expect(after[1]).toBe(before[0]);
        expect(after.filter(t => t !== before[0])).toEqual(before.filter(t => t !== before[0]));
    });

    it("Alt+方向键调整顺序", async () => {
        await importFixtures();
        const before = await queueRows();
        const rows = await browser.$$(".q-row");
        await rows[1].click();
        await browser.keys(["Alt", "ArrowDown"]);
        await browser.waitUntil(async () => (await queueRows()).join("|") !== before.join("|"), {
            timeoutMsg: "Alt+↓ 后顺序未变化",
        });
        const after = await queueRows();
        expect(after[2]).toBe(before[1]);
    });

    it("搜索时不能排序", async () => {
        await importFixtures();
        const before = await queueRows();
        await browser.execute(() => (document.querySelector('[aria-label="搜索"]') as HTMLElement).click());
        await browser.waitUntil(
            async () => await browser.execute(() => Boolean(document.querySelector(".q-search input"))),
        );
        await browser.execute(() => {
            const input = document.querySelector<HTMLInputElement>(".q-search input")!;
            // React 受控输入：必须用原型 setter 赋值再派发 input 事件
            const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
            setter.call(input, "track");
            input.dispatchEvent(new Event("input", { bubbles: true }));
        });
        await browser.waitUntil(async () => (await queueRows()).length < before.length, { timeoutMsg: "搜索未过滤" });

        const filtered = await browser.$$(".q-row");
        await filtered[0].click();
        await browser.keys(["Alt", "ArrowDown"]);
        await browser.pause(300);

        // 取消搜索后顺序不变
        await browser.execute(() => (document.querySelector(".q-cancel") as HTMLElement).click());
        await browser.waitUntil(async () => (await queueRows()).length === before.length);
        expect(await queueRows()).toEqual(before);
    });
});
