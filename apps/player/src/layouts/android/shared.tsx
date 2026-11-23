import type { ReactNode } from "react";
import { useLayoutEffect, useRef } from "react";
import type { TrackView } from "../../ipc";

/** 内置的「全部音乐」就是曲库本身，固定排在歌单列表第一位，不作为歌单保存。 */
export const ALL_MUSIC = { id: "all", name: "全部音乐", colors: ["#2440b8", "#eef0ea"] as const };

/** 页面进出动画的时长（毫秒），与 android.css 里的 transition 一致。 */
export const PAGE_MS = 340;
const PAGE_EASING = "cubic-bezier(0.22, 0.8, 0.24, 1)";

export function reduceMotion() {
    return typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

export function Icon({ path }: { path: string }) {
    return (
        <svg className="m-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d={path} />
        </svg>
    );
}

export const icons = {
    play: "M8 5.5v13a1 1 0 0 0 1.5.86l10.5-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z",
    pause: "M6 5h4v14H6zM14 5h4v14h-4z",
    prev: "M5 5h2.2v14H5zM20 6.2v11.6a.8.8 0 0 1-1.2.7L10 12.7a.8.8 0 0 1 0-1.4l8.8-5.8a.8.8 0 0 1 1.2.7z",
    next: "M16.8 5H19v14h-2.2zM4 6.2v11.6a.8.8 0 0 0 1.2.7L14 12.7a.8.8 0 0 0 0-1.4L5.2 5.5A.8.8 0 0 0 4 6.2z",
    close: "M6 6l12 12M18 6 6 18",
    down: "M5 9l7 7 7-7",
    // 竖排两点“⋮”
    dots: "M12 7.5h.01M12 16.5h.01",
    more: "M5 12h.01M12 12h.01M19 12h.01",
    queue: "M4 6h16M4 11h16M4 16h8M15.5 14l5 3-5 3z",
    plus: "M12 5v14M5 12h14",
    back: "M19 12H5M12 19l-7-7 7-7",
    // 音符的两个圆头用两段弧画出：Icon 只画一个 path
    note: "M9 18V6l11-2v12M4 18a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0M15 16a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0",
    folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
    search: "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM21 21l-4.35-4.35",
    check: "M4 12l5 5L20 6",
    edit: "M4 20h4L19 9a2.1 2.1 0 0 0-3-3L5 17v3z",
    trash: "M4 7h16M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2M6 7l1 13a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1l1-13M10 11v6M14 11v6",
};

/** 搜索匹配歌名或歌手，不区分大小写；空串匹配全部。 */
export function matches(track: TrackView, query: string) {
    const needle = query.trim().toLowerCase();
    return !needle || `${track.title}${track.artist}`.toLowerCase().includes(needle);
}

/** 歌名、纯音乐标签和歌手。 */
export function TrackText({ track }: { track: TrackView }) {
    return (
        <span className="tx">
            <span className="t">{track.title}</span>
            <span className="a">
                {!track.hasLyrics && <span className="m-tag">纯音乐</span>}
                <span>{track.artist}</span>
            </span>
        </span>
    );
}

/** 正在播放：封面上的均衡条。 */
export function Equalizer({ className = "eq" }: { className?: string }) {
    return (
        <span className={className} aria-hidden="true">
            <i />
            <i />
            <i />
            <i />
        </span>
    );
}

export function SearchBox({ value, onChange }: { value: string; onChange: (value: string) => void }) {
    return (
        <div className="lib-top">
            <label className="m-search">
                <Icon path={icons.search} />
                <input
                    placeholder="搜索歌名或歌手"
                    aria-label="搜索歌名或歌手"
                    autoComplete="off"
                    value={value}
                    onChange={event => onChange(event.target.value)}
                />
            </label>
        </div>
    );
}

export type PageProps = {
    level: number;
    /** 是否是栈顶：被盖住或正在退出的页面不参与点击、焦点和无障碍树。 */
    active: boolean;
    leaving: boolean;
};

/**
 * 页面栈里的一页：铺满屏幕、按栈序叠放。进入时从右侧滑入（播放页从底部升起），
 * 退出时由 `is-leaving` 滑出，随后被移除。系统栏让位由内层的 `.page-body` 负责。
 */
export function Page({
    kind,
    level,
    active,
    leaving,
    label,
    children,
}: PageProps & { kind: string; label: string; children: ReactNode }) {
    const element = useRef<HTMLElement>(null);
    useLayoutEffect(() => {
        const node = element.current;
        if (!node || typeof node.animate !== "function" || reduceMotion()) return;
        const from = kind === "player" ? "translateY(100%)" : "translateX(100%)";
        node.animate([{ transform: from }, { transform: "none" }], {
            duration: PAGE_MS,
            easing: PAGE_EASING,
        });
    }, [kind]);
    return (
        <section
            ref={element}
            data-page={kind}
            aria-label={label}
            aria-hidden={!active}
            inert={!active}
            className={`page page--${kind}${leaving ? " is-leaving" : ""}`}
            style={{ zIndex: 10 + level }}
        >
            {children}
        </section>
    );
}

/** 返回键。 */
export function BackButton({ onClick }: { onClick: () => void }) {
    return (
        <button type="button" className="top-btn" aria-label="返回" title="返回" onClick={onClick}>
            <Icon path={icons.back} />
        </button>
    );
}
