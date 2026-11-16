import { useCallback, useEffect, useLayoutEffect, useRef } from "react";
import "./LyricView.css";

export type LyricLine = { timeMs: number; text: string };
export type LyricViewProps = {
    lines: LyricLine[];
    positionMs: number;
    onSeek: (ms: number) => void;
    className?: string;
};

function prefersReducedMotion() {
    return typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** 当前可见的歌词行及其位置；只动画可见行，长歌词也只处理十几行。 */
function visibleLineRects(view: HTMLElement | null) {
    const rects = new Map<HTMLElement, DOMRect>();
    if (!view) return rects;
    const box = view.getBoundingClientRect();
    for (const line of view.querySelectorAll<HTMLElement>(".ly")) {
        const rect = line.getBoundingClientRect();
        if (rect.bottom >= box.top && rect.top <= box.bottom) rects.set(line, rect);
    }
    return rects;
}

export function LyricView({ lines, positionMs, onSeek, className = "" }: LyricViewProps) {
    const container = useRef<HTMLDivElement>(null);
    const pauseTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
    // 居左/居中切换（展开或收起队列）时，记下切换前各行的位置，提交后让它们从旧位置滑到新位置
    const layoutClass = useRef(className);
    const before = useRef<Map<HTMLElement, DOMRect> | null>(null);
    if (layoutClass.current !== className) {
        layoutClass.current = className;
        before.current = visibleLineRects(container.current);
    }
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
        view.scrollTo({ top, behavior: prefersReducedMotion() ? "auto" : "smooth" });
    }, []);

    useLayoutEffect(() => {
        if (pauseTimer.current === null && lines.length) recenter();
    }, [active, lines, recenter]);

    useLayoutEffect(() => {
        const rects = before.current;
        before.current = null;
        if (!rects || prefersReducedMotion()) return;
        for (const [line, old] of rects) {
            if (typeof line.animate !== "function") continue;
            const now = line.getBoundingClientRect();
            const dx = Math.round(old.left - now.left);
            const dy = Math.round(old.top - now.top);
            if (!dx && !dy) continue;
            // 用独立的 translate 属性，不覆盖当前行的 scale
            line.animate([{ translate: `${dx}px ${dy}px` }, { translate: "0px 0px" }], {
                duration: 500,
                easing: "cubic-bezier(0.22, 0.8, 0.24, 1)",
            });
        }
    }, [className]);

    useEffect(
        () => () => {
            if (pauseTimer.current !== null) clearTimeout(pauseTimer.current);
        },
        [],
    );

    // 布局变化（收起队列、调整窗口、换行数改变）后重新把当前行放回原位，与原型一致
    useEffect(() => {
        const view = container.current;
        if (!view || typeof ResizeObserver === "undefined") return;
        const observer = new ResizeObserver(() => {
            if (pauseTimer.current === null) recenter();
        });
        observer.observe(view);
        return () => observer.disconnect();
    }, [recenter]);

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
