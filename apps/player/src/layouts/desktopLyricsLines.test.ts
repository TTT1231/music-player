import { expect, test } from "vitest";
import { desktopLyricsLines, INSTRUMENTAL, lineShift } from "./desktopLyricsLines";

const lines = [
    { timeMs: 1_000, text: "第一句" },
    { timeMs: 3_000, text: "第二句" },
    { timeMs: 5_000, text: "第三句" },
];
const base = {
    title: "歌曲",
    artist: "歌手",
    hasLyrics: true,
    loaded: true,
    lines,
    positionMs: 2_000,
    durationMs: 10_000 as number | null,
};

test("没有当前歌曲时居中显示应用名", () => {
    const { a, b } = desktopLyricsLines({ ...base, title: null, hasLyrics: false, loaded: true, lines: [] });
    expect(a).toMatchObject({ text: "music", solo: true, idle: false, k: 1 });
    expect(b.text).toBe("");
});

test("没有歌词时居中显示“纯音乐，请欣赏”", () => {
    const { a } = desktopLyricsLines({ ...base, hasLyrics: false, loaded: true, lines: [] });
    expect(a).toMatchObject({ text: INSTRUMENTAL, solo: true, k: 1 });
});

test("歌词还在加载时不显示任何一行", () => {
    const { a, b } = desktopLyricsLines({ ...base, loaded: false });
    expect(a.text).toBe("");
    expect(b.text).toBe("");
});

test("歌词存在但没有一行时居中显示歌名和歌手", () => {
    const { a } = desktopLyricsLines({ ...base, lines: [] });
    expect(a).toMatchObject({ text: "歌曲 - 歌手", solo: true, k: 1 });
});

test("前奏时第一行显示歌名和歌手，第二行预告第一句", () => {
    const { a, b } = desktopLyricsLines({ ...base, positionMs: 0 });
    expect(a).toMatchObject({ text: "歌曲 - 歌手", idle: true });
    expect(b).toMatchObject({ text: "第一句", idle: true });
});

test("当前行按奇偶交替落到两行，下一行以空闲样式预告", () => {
    const first = desktopLyricsLines({ ...base, positionMs: 2_000 });
    expect(first.a).toMatchObject({ text: "第一句", idle: false });
    expect(first.b).toMatchObject({ text: "第二句", idle: true });

    const second = desktopLyricsLines({ ...base, positionMs: 4_000 });
    expect(second.b).toMatchObject({ text: "第二句", idle: false });
    expect(second.a).toMatchObject({ text: "第三句", idle: true });
});

test("当前行的着色进度按行内时间推进，并限制在 0–1", () => {
    // 第一句 1s–3s：2s 处应着色一半
    expect(desktopLyricsLines({ ...base, positionMs: 2_000 }).a.k).toBeCloseTo(0.5);
    // 原型提前 0.15s 进入下一行：2.84s 时第一句仍是当前行
    expect(desktopLyricsLines({ ...base, positionMs: 2_840 }).a.k).toBeCloseTo(0.92);
    // 超出时长后最后一行的着色进度封顶为 1
    expect(desktopLyricsLines({ ...base, positionMs: 10_500 }).a.k).toBe(1);
    expect(desktopLyricsLines({ ...base, positionMs: 0 }).a.k).toBe(0);
});

test("最后一行的着色进度按时长或 5 秒兜底计算", () => {
    const end = desktopLyricsLines({ ...base, positionMs: 7_000, durationMs: 9_000 });
    expect(end.a).toMatchObject({ text: "第三句", k: 0.5 });
    const fallback = desktopLyricsLines({ ...base, positionMs: 5_500, durationMs: null });
    // 5s + 5s 兜底：5.5s 处应为 0.1
    expect(fallback.a.k).toBeCloseTo(0.1);
});

test("an over-long current line slides so the sung position stays in view", () => {
    // 放得下：不移动
    expect(lineShift({ k: 0.9, textWidth: 800, boxWidth: 900 })).toBe(0);
    // 刚开始唱：先显示开头
    expect(lineShift({ k: 0.1, textWidth: 1800, boxWidth: 900 })).toBe(0);
    // 唱到中间：唱到的位置保持在可视宽度约 70% 处
    expect(lineShift({ k: 0.5, textWidth: 1800, boxWidth: 900 })).toBe(270);
    // 唱到末尾：最多移到行尾对齐右边，不留空白
    expect(lineShift({ k: 1, textWidth: 1800, boxWidth: 900 })).toBe(900);
});
