import type { ImportResult } from "../ipc";

export function importSummary(result: ImportResult): string {
    const { added, withLyrics, lyricsAttached, duplicates, sourcesKept } = result;
    const parts: string[] = [];
    if (added.length) {
        const lyrics = withLyrics + lyricsAttached;
        parts.push(`已导入 ${added.length} 首${lyrics ? `，${lyrics} 首有歌词` : ""}`);
    } else if (lyricsAttached) {
        parts.push(`已为 ${lyricsAttached} 首歌加上歌词`);
    }
    if (duplicates) parts.push(`已跳过 ${duplicates} 个同名文件`);
    if (sourcesKept) parts.push(`${sourcesKept} 个文件已复制，原位置的文件仍保留`);
    return parts.length ? parts.join("。") : "没有找到歌曲，请重新选择";
}
