import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import type { LyricLine } from "../components/LyricView";
import { commands } from "../ipc";
import { usePlaybackPosition, usePlayerState } from "../store";
import { desktopLyricsLines, lineShift } from "./desktopLyricsLines";
import "./DesktopLyrics.css";

const WIDTH = 980;
const icons = {
    play: "M8 5.5v13a1 1 0 0 0 1.5.86l10.5-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z",
    pause: "M6 5h4v14H6zM14 5h4v14h-4z",
    prev: "M5 5h2.2v14H5zM20 6.2v11.6a.8.8 0 0 1-1.2.7L10 12.7a.8.8 0 0 1 0-1.4l8.8-5.8a.8.8 0 0 1 1.2.7z",
    next: "M16.8 5H19v14h-2.2zM4 6.2v11.6a.8.8 0 0 0 1.2.7L14 12.7a.8.8 0 0 0 0-1.4L5.2 5.5A.8.8 0 0 0 4 6.2z",
    close: "M6 6l12 12M18 6 6 18",
};
function Icon({ path }: { path: string }) {
    return (
        <svg className="dl-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d={path} />
        </svg>
    );
}

/** 桌面歌词窗口：透明、置顶、可拖动的独立窗口，两行交替、当前行按进度着色。 */
export function DesktopLyrics() {
    const { state } = usePlayerState();
    const position = usePlaybackPosition();
    const [lines, setLines] = useState<LyricLine[] | null>(null);
    const [fontSize, setFontSize] = useState(32);
    const desk = useRef<HTMLDivElement>(null);
    const current = state?.tracks.find(track => track.id === state.currentId);

    useEffect(() => {
        let live = true;
        setLines(null);
        if (current?.hasLyrics) {
            void commands
                .getLyrics(current.id)
                .then(value => {
                    if (live) setLines(value.lines);
                })
                .catch(() => {
                    if (live) setLines([]);
                });
        }
        return () => {
            live = false;
        };
    }, [current?.id, current?.hasLyrics]);

    const rendered = desktopLyricsLines({
        title: current?.title ?? null,
        artist: current?.artist ?? "",
        hasLyrics: Boolean(current?.hasLyrics),
        loaded: !current?.hasLyrics || lines !== null,
        lines: lines ?? [],
        positionMs: position,
        durationMs: state?.durationMs ?? null,
    });

    // 歌词区高度随内容（行数、字号）变化，窗口高度跟着贴合内容
    useLayoutEffect(() => {
        const element = desk.current;
        if (!element) return;
        const height = Math.ceil(element.getBoundingClientRect().height);
        if (height <= 0) return;
        try {
            void getCurrentWindow()
                .setSize(new LogicalSize(WIDTH, height))
                .catch(() => {});
        } catch {
            /* 组件测试等无窗口环境跳过。 */
        }
    }, [fontSize, rendered.a.text, rendered.b.text]);

    // 比窗口宽的当前行跟着唱到的位置左移；空闲行显示开头并以省略号收尾
    useLayoutEffect(() => {
        for (const line of desk.current?.querySelectorAll<HTMLElement>(".dl") ?? []) {
            const text = line.querySelector<HTMLElement>(".base");
            if (!text) continue;
            const k = line.classList.contains("idle") ? 0 : rendered[line.classList.contains("a") ? "a" : "b"].k;
            const shift = lineShift({ k, textWidth: text.scrollWidth, boxWidth: line.clientWidth });
            line.style.setProperty("--shift", `${-shift}px`);
            line.classList.toggle("sliding", shift > 0);
        }
    });

    const run = (promise: Promise<unknown>) => void promise.catch(() => {});
    const step = (delta: number) => setFontSize(size => Math.min(60, Math.max(22, size + delta)));
    const lineClass = (side: string, line: { idle: boolean; solo: boolean }) =>
        `dl ${side}${line.idle ? " idle" : ""}${line.solo ? " solo" : ""}`;
    return (
        <div
            ref={desk}
            className="desk"
            data-testid="desk"
            data-tauri-drag-region
            style={{ "--fs": `${fontSize}px` } as CSSProperties}
        >
            <div className="desk-bar" data-tauri-drag-region>
                <button type="button" aria-label="上一首" onClick={() => current && run(commands.prev())}>
                    <Icon path={icons.prev} />
                </button>
                <button
                    type="button"
                    aria-label={state?.playing ? "暂停" : "播放"}
                    onClick={() => current && run(commands.toggle())}
                >
                    <Icon path={state?.playing ? icons.pause : icons.play} />
                </button>
                <button type="button" aria-label="下一首" onClick={() => current && run(commands.next())}>
                    <Icon path={icons.next} />
                </button>
                <span className="sep" />
                <button type="button" aria-label="缩小字号" title="缩小字号" onClick={() => step(-4)}>
                    A-
                </button>
                <button type="button" aria-label="放大字号" title="放大字号" onClick={() => step(4)}>
                    A+
                </button>
                <button
                    type="button"
                    className="close"
                    aria-label="关闭桌面歌词"
                    title="关闭桌面歌词"
                    onClick={() => run(commands.toggleDesktopLyrics())}
                >
                    <Icon path={icons.close} />
                </button>
            </div>
            <div className="desk-lines" data-tauri-drag-region>
                {(
                    [
                        ["a", rendered.a],
                        ["b", rendered.b],
                    ] as const
                ).map(([side, line]) => (
                    <div
                        key={side}
                        className={lineClass(side, line)}
                        data-tauri-drag-region
                        hidden={!line.text}
                        style={{ "--k": `${(line.k * 100).toFixed(1)}%` } as CSSProperties}
                    >
                        <span className="dl-text">
                            <span className="base">{line.text}</span>
                            <span className="fill" aria-hidden="true">
                                {line.text}
                            </span>
                        </span>
                    </div>
                ))}
            </div>
        </div>
    );
}
