import { useCallback, useEffect, useLayoutEffect, useRef } from "react";
import "./LyricView.css";

export type LyricLine = { timeMs: number; text: string };
export type LyricViewProps = {
    lines: LyricLine[];
    positionMs: number;
    onSeek: (ms: number) => void;
    className?: string;
};

export function LyricView({ lines, positionMs, onSeek, className = "" }: LyricViewProps) {
    const container = useRef<HTMLDivElement>(null);
    const pauseTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
    let active = -1;
    for (let index = 0; index < lines.length; index++) {
        if (lines[index].timeMs <= positionMs) active = index;
        else break;
    }

    const recenter = useCallback(() => {
        const view = container.current;
        const line =
            view?.querySelector<HTMLElement>(".ly.is-active") ?? view?.querySelector<HTMLElement>(".ly[data-time]");
        if (!view || !line || typeof view.scrollTo !== "function") return;
        const offset = line.getBoundingClientRect().top - view.getBoundingClientRect().top + view.scrollTop;
        const top = offset - view.clientHeight * 0.42 + line.offsetHeight / 2;
        const reduced =
            typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
        view.scrollTo({ top, behavior: reduced ? "auto" : "smooth" });
    }, []);

    useLayoutEffect(() => {
        if (pauseTimer.current === null && lines.length) recenter();
    }, [active, lines, recenter]);

    useEffect(
        () => () => {
            if (pauseTimer.current !== null) clearTimeout(pauseTimer.current);
        },
        [],
    );

    const manualScroll = () => {
        if (pauseTimer.current !== null) clearTimeout(pauseTimer.current);
        pauseTimer.current = setTimeout(() => {
            pauseTimer.current = null;
            recenter();
        }, 3000);
    };

    const seek = (ms: number) => {
        if (pauseTimer.current !== null) clearTimeout(pauseTimer.current);
        pauseTimer.current = null;
        onSeek(ms);
    };

    return (
        <div
            ref={container}
            className={`lyrics ${className}`.trim()}
            aria-label="歌词"
            onWheel={manualScroll}
            onTouchMove={manualScroll}
            onPointerDown={event => {
                if (event.target === event.currentTarget) manualScroll();
            }}
            onKeyDown={event => {
                if (["ArrowUp", "ArrowDown", "PageUp", "PageDown", "Home", "End"].includes(event.key)) manualScroll();
            }}
        >
            <div className="pad" aria-hidden="true" />
            {lines.length === 0 ? (
                <p className="ly is-active instrumental">纯音乐，请欣赏</p>
            ) : (
                lines.map((line, index) => (
                    <button
                        type="button"
                        className={`ly${index === active ? " is-active" : index < active ? " past" : ""}`}
                        data-time={line.timeMs}
                        key={`${line.timeMs}-${index}`}
                        onClick={() => seek(line.timeMs)}
                    >
                        {line.text}
                    </button>
                ))
            )}
            {lines.length > 0 && <div className="pad" aria-hidden="true" />}
        </div>
    );
}
