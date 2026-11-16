import type { LyricLine } from "../components/LyricView";

export const INSTRUMENTAL = "纯音乐，请欣赏";
export const BRAND = "music";

export type DesktopLyricLine = {
    text: string;
    /** 空闲行：不逐字着色（下一句预告、前奏提示） */
    idle: boolean;
    /** 独占一行并居中（应用名、纯音乐、整行显示歌名） */
    solo: boolean;
    /** 当前行已唱过的比例 0–1，驱动逐字着色 */
    k: number;
};

const hidden: DesktopLyricLine = { text: "", idle: false, solo: false, k: 0 };

/** 桌面歌词窗口的两行内容；逻辑逐条对照原型 core.js / desktop.html 的 renderDesk 与 activeLine。 */
export function desktopLyricsLines(input: {
    title: string | null;
    artist: string;
    hasLyrics: boolean;
    loaded: boolean;
    lines: LyricLine[];
    positionMs: number;
    durationMs: number | null;
}): { a: DesktopLyricLine; b: DesktopLyricLine } {
    const solo = (text: string) => ({ a: { text, idle: false, solo: true, k: 1 }, b: { ...hidden } });
    if (!input.title) return solo(BRAND);
    if (!input.loaded) return { a: { ...hidden }, b: { ...hidden } };
    if (!input.hasLyrics) return solo(INSTRUMENTAL);
    const lines = input.lines;
    if (!lines.length) return solo(`${input.title} - ${input.artist}`);
    // 原型 activeLine：提前 0.15s 进入下一行
    const time = input.positionMs + 150;
    let active = -1;
    for (let index = 0; index < lines.length && lines[index].timeMs <= time; index++) active = index;
    if (active < 0) {
        // 前奏：第一行显示歌名，第二行预告第一句
        return {
            a: { text: `${input.title} - ${input.artist}`, idle: true, solo: false, k: 0 },
            b: { text: lines[0].text, idle: true, solo: false, k: 0 },
        };
    }
    const current = lines[active];
    const next = lines[active + 1];
    const end = next ? next.timeMs : input.durationMs || current.timeMs + 5_000;
    const k = Math.min(1, Math.max(0, (input.positionMs - current.timeMs) / Math.max(300, end - current.timeMs)));
    const mine: DesktopLyricLine = { text: current.text, idle: false, solo: false, k };
    const other: DesktopLyricLine = { text: next ? next.text : "", idle: true, solo: false, k: 0 };
    return active % 2 === 0 ? { a: mine, b: other } : { a: other, b: mine };
}

/**
 * 当前行比窗口宽时向左平移的距离（px）：唱到的位置保持在可视宽度约 70% 处，
 * 开头不移动，最多移到行尾贴住右边。放得下时为 0。
 */
export function lineShift({ k, textWidth, boxWidth }: { k: number; textWidth: number; boxWidth: number }): number {
    const overflow = textWidth - boxWidth;
    if (overflow <= 0) return 0;
    return Math.round(Math.min(overflow, Math.max(0, k * textWidth - boxWidth * 0.7)));
}
