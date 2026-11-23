import { memo, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { Cover } from "../components/Cover";
import { formatTime } from "../components/formatTime";
import { useSmoothWheel } from "../components/useSmoothWheel";
import type { TrackView } from "../ipc";
import "./QueuePanel.css";

export type QueuePanelProps = {
    tracks: TrackView[];
    currentId: string | null;
    playing: boolean;
    open: boolean;
    coverRevisions?: Readonly<Record<string, number>>;
    locateRequest?: { id: string; version: number };
    onPlay: (id: string) => void;
    onRemove: (id: string) => void;
    onMove: (from: number, to: number) => void;
    onImport: () => void;
    onClose: () => void;
    onShowLyrics?: (id?: string) => void;
};

type Drag = {
    row: HTMLElement;
    rows: HTMLElement[];
    startY: number;
    y: number;
    from: number;
    to: number;
    height: number;
    scrollStart: number;
    started: boolean;
    raf: number | null;
};

function Icon({ name }: { name: "compact" | "covers" | "close" | "search" | "plus" }) {
    const shape = {
        compact: (
            <>
                <rect x="4" y="4.5" width="5" height="5" rx=".8" />
                <rect x="4" y="14.5" width="5" height="5" rx=".8" />
                <path d="M12.5 6h7.5M12.5 9h5M12.5 16h7.5M12.5 19h5" />
            </>
        ),
        covers: (
            <>
                <path d="M9.5 7h10.5M9.5 12h10.5M9.5 17h10.5" />
                <path d="M4.6 7h.4M4.6 12h.4M4.6 17h.4" strokeWidth="2.6" />
            </>
        ),
        close: <path d="M6 6l12 12M18 6 6 18" />,
        search: (
            <>
                <circle cx="11" cy="11" r="7" />
                <path d="m20 20-3.5-3.5" />
            </>
        ),
        plus: <path d="M12 5v14M5 12h14" />,
    };
    return (
        <svg className="q-icon" viewBox="0 0 24 24" aria-hidden="true">
            {shape[name]}
        </svg>
    );
}

function highlighted(text: string, query: string): ReactNode {
    if (!query) return text;
    const lower = text.toLowerCase();
    const needle = query.toLowerCase();
    const parts: ReactNode[] = [];
    let start = 0;
    let found = lower.indexOf(needle, start);
    while (found >= 0) {
        parts.push(text.slice(start, found), <mark key={found}>{text.slice(found, found + needle.length)}</mark>);
        start = found + needle.length;
        found = lower.indexOf(needle, start);
    }
    parts.push(text.slice(start));
    return parts;
}

function Equalizer() {
    return (
        <span className="eq" aria-hidden="true">
            <i />
            <i />
            <i />
            <i />
        </span>
    );
}

export const QueuePanel = memo(function QueuePanel({
    tracks,
    currentId,
    playing,
    open,
    coverRevisions,
    locateRequest,
    onPlay,
    onRemove,
    onMove,
    onImport,
    onClose,
    onShowLyrics,
}: QueuePanelProps) {
    const [compact, setCompact] = useState(false);
    const [searching, setSearching] = useState(false);
    const [query, setQuery] = useState("");
    const listRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLInputElement>(null);
    const searchButton = useRef<HTMLButtonElement>(null);
    const drag = useRef<Drag | null>(null);
    const focusRow = useRef<string | null>(null);
    const onMoveRef = useRef(onMove);
    const anchor = useRef<{ id: string; offset: number } | null>(null);
    const located = useRef(0);
    useSmoothWheel(listRef);
    const q = query.trim();
    const view = tracks.filter(track => !q || (track.title + track.artist).toLowerCase().includes(q.toLowerCase()));
    const indexOf = new Map(tracks.map((track, index) => [track.id, index]));

    useEffect(() => {
        onMoveRef.current = onMove;
    }, [onMove]);
    useEffect(() => {
        if (searching) inputRef.current?.focus();
    }, [searching]);
    useEffect(() => {
        // 移除后把焦点落到原来的下一行（末尾则上一行），方便连续删除
        const id = focusRow.current;
        if (!id) return;
        focusRow.current = null;
        const row = Array.from(listRef.current?.querySelectorAll<HTMLElement>(".q-row") ?? []).find(
            node => node.dataset.id === id,
        );
        row?.focus();
    }, [tracks]);

    // 每个定位请求只滚动一次；歌曲可能稍后才出现在列表里，所以列表变化时再找一次
    useEffect(() => {
        const list = listRef.current;
        if (!list || !locateRequest || located.current === locateRequest.version) return;
        const row = Array.from(list.querySelectorAll<HTMLElement>(".q-row")).find(
            node => node.dataset.id === locateRequest.id,
        );
        if (!row) return;
        located.current = locateRequest.version;
        if (typeof list.scrollTo === "function") {
            list.scrollTo({
                top: row.offsetTop - list.offsetTop - list.clientHeight / 2 + row.offsetHeight / 2,
                behavior: "smooth",
            });
        }
    }, [locateRequest, tracks, searching, query]);

    // 切换紧凑/封面列表时，切换前第一行完整可见的歌曲留在原来的位置，列表不跳动
    useLayoutEffect(() => {
        const list = listRef.current;
        const saved = anchor.current;
        anchor.current = null;
        if (!list || !saved) return;
        const row = Array.from(list.querySelectorAll<HTMLElement>(".q-row")).find(node => node.dataset.id === saved.id);
        if (row) list.scrollTop = row.offsetTop - saved.offset;
    }, [compact]);
    const toggleCompact = () => {
        const list = listRef.current;
        if (list) {
            const row = Array.from(list.querySelectorAll<HTMLElement>(".q-row")).find(
                node => node.offsetTop - list.offsetTop >= list.scrollTop,
            );
            if (row?.dataset.id) anchor.current = { id: row.dataset.id, offset: row.offsetTop - list.scrollTop };
        }
        setCompact(value => !value);
    };

    useEffect(() => {
        const list = listRef.current;
        if (!list) return;
        const layout = () => {
            const d = drag.current;
            if (!d?.started) return;
            const offset = d.y - d.startY + list.scrollTop - d.scrollStart;
            d.row.style.transform = `translateY(${offset}px)`;
            d.to = Math.max(0, Math.min(d.rows.length - 1, d.from + Math.round(offset / d.height)));
            d.rows.forEach((row, index) => {
                if (row === d.row) return;
                const shift =
                    d.from < d.to && index > d.from && index <= d.to
                        ? -d.height
                        : d.to < d.from && index >= d.to && index < d.from
                          ? d.height
                          : 0;
                row.style.transform = shift ? `translateY(${shift}px)` : "";
            });
        };
        const tick = () => {
            const d = drag.current;
            if (!d?.started) return;
            const rect = list.getBoundingClientRect();
            const velocity =
                d.y < rect.top + 48 ? -(rect.top + 48 - d.y) : d.y > rect.bottom - 48 ? d.y - (rect.bottom - 48) : 0;
            if (velocity) {
                list.scrollTop += Math.max(-18, Math.min(18, velocity / 3));
                layout();
            }
            d.raf = requestAnimationFrame(tick);
        };
        const move = (event: PointerEvent) => {
            const d = drag.current;
            if (!d) return;
            d.y = event.clientY;
            if (!d.started) {
                if (Math.abs(d.y - d.startY) < 5) return;
                d.started = true;
                d.height = d.row.offsetHeight || 1;
                d.scrollStart = list.scrollTop;
                d.row.classList.add("dragging");
                list.classList.add("q-sorting");
                window.getSelection()?.removeAllRanges();
                d.raf = requestAnimationFrame(tick);
            }
            layout();
        };
        const finish = (commit: boolean) => {
            const d = drag.current;
            drag.current = null;
            if (!d?.started) return;
            if (d.raf !== null) cancelAnimationFrame(d.raf);
            list.classList.remove("q-sorting");
            d.rows.forEach(row => {
                row.style.transform = "";
            });
            d.row.classList.remove("dragging");
            if (commit && d.from !== d.to) onMoveRef.current(d.from, d.to);
        };
        const up = () => finish(true);
        const cancel = () => finish(false);
        window.addEventListener("pointermove", move);
        window.addEventListener("pointerup", up);
        window.addEventListener("pointercancel", cancel);
        window.addEventListener("blur", cancel);
        return () => {
            cancel();
            window.removeEventListener("pointermove", move);
            window.removeEventListener("pointerup", up);
            window.removeEventListener("pointercancel", cancel);
            window.removeEventListener("blur", cancel);
        };
    }, []);

    const closeSearch = () => {
        setSearching(false);
        setQuery("");
        searchButton.current?.focus();
    };
    return (
        <aside
            className={`queue${open ? "" : " queue-closed"}${playing ? "" : " paused"}`}
            aria-label="播放队列"
            aria-hidden={!open}
        >
            <div className="q-head">
                <h2 className="q-title">
                    {onShowLyrics ? (
                        <button type="button" onClick={() => onShowLyrics()}>
                            播放队列
                        </button>
                    ) : (
                        "播放队列"
                    )}
                </h2>
                <div className="q-tools">
                    <button
                        type="button"
                        className="q-btn"
                        aria-label={compact ? "显示封面" : "紧凑列表"}
                        title={compact ? "显示封面" : "紧凑列表"}
                        onClick={toggleCompact}
                    >
                        <Icon name={compact ? "covers" : "compact"} />
                    </button>
                    <button type="button" className="q-btn" aria-label="收起播放队列" title="收起" onClick={onClose}>
                        <Icon name="close" />
                    </button>
                </div>
            </div>
            <div className="q-sub">
                <span>{q ? `找到 ${view.length} 首` : `共${tracks.length}首`}</span>
                <div className="q-tools">
                    <button
                        ref={searchButton}
                        type="button"
                        className="q-btn"
                        aria-label="搜索"
                        title="搜索"
                        aria-pressed={searching}
                        onClick={() => (searching ? closeSearch() : setSearching(true))}
                    >
                        <Icon name="search" />
                    </button>
                    <button
                        type="button"
                        className="q-btn"
                        aria-label="导入音乐和歌词"
                        title="导入音乐和歌词"
                        onClick={onImport}
                    >
                        <Icon name="plus" />
                    </button>
                </div>
            </div>
            {searching && (
                <div className="q-search-row">
                    <div className="q-search">
                        <Icon name="search" />
                        <input
                            ref={inputRef}
                            placeholder="搜索歌名或歌手"
                            aria-label="搜索歌名或歌手"
                            autoComplete="off"
                            value={query}
                            onChange={event => setQuery(event.target.value)}
                            onKeyDown={event => {
                                if (event.key === "Escape") {
                                    event.stopPropagation();
                                    closeSearch();
                                }
                            }}
                        />
                        {query && (
                            <button
                                type="button"
                                className="q-clear"
                                aria-label="清空搜索"
                                onClick={() => {
                                    setQuery("");
                                    inputRef.current?.focus();
                                }}
                            >
                                <Icon name="close" />
                            </button>
                        )}
                    </div>
                    <button type="button" className="q-cancel" onClick={closeSearch}>
                        取消
                    </button>
                </div>
            )}
            <div ref={listRef} className={`q-list${compact ? " compact" : ""}`} role="list">
                {view.length === 0 ? (
                    <p className="q-empty">
                        {tracks.length
                            ? "没有找到匹配的歌曲。"
                            : "还没有歌曲。点上面的 + 导入音乐和歌词，或把文件拖进窗口。"}
                    </p>
                ) : (
                    view.map((track, index) => {
                        const originalIndex = indexOf.get(track.id) ?? index;
                        const current = track.id === currentId;
                        return (
                            <div
                                key={track.id}
                                role="listitem"
                                tabIndex={0}
                                data-id={track.id}
                                className={`q-row${current ? " on" : ""}`}
                                onClick={() => onShowLyrics?.(track.id)}
                                onDoubleClick={() => onPlay(track.id)}
                                onKeyDown={event => {
                                    if (event.key === "Enter") onPlay(track.id);
                                    if (event.key === "Delete") {
                                        event.preventDefault();
                                        const at = view.findIndex(item => item.id === track.id);
                                        focusRow.current = view[at + 1]?.id ?? view[at - 1]?.id ?? null;
                                        onRemove(track.id);
                                    }
                                    if (event.altKey && !q && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
                                        event.preventDefault();
                                        const target = originalIndex + (event.key === "ArrowUp" ? -1 : 1);
                                        if (target >= 0 && target < tracks.length) onMove(originalIndex, target);
                                    }
                                }}
                                onPointerDown={event => {
                                    if (event.button !== 0 || q) return;
                                    const list = listRef.current;
                                    if (!list) return;
                                    const row = event.currentTarget;
                                    const rows = Array.from(list.querySelectorAll<HTMLElement>(".q-row"));
                                    drag.current = {
                                        row,
                                        rows,
                                        startY: event.clientY,
                                        y: event.clientY,
                                        from: originalIndex,
                                        to: originalIndex,
                                        height: 0,
                                        scrollStart: list.scrollTop,
                                        started: false,
                                        raf: null,
                                    };
                                }}
                            >
                                <span className="n">
                                    <span className="num">
                                        {compact ? String(index + 1).padStart(2, "0") : index + 1}
                                    </span>
                                    {compact && <Equalizer />}
                                </span>
                                {/* 紧凑模式只隐藏封面，不卸载：切回时图片无需重新加载 */}
                                <span className="cv">
                                    <Cover
                                        id={track.id}
                                        title={track.title}
                                        coverUrl={track.coverUrl}
                                        revision={coverRevisions?.[track.id]}
                                        size={48}
                                    />
                                    <Equalizer />
                                </span>
                                <div className="q-text">
                                    <div className="t">
                                        {highlighted(track.title, q)}
                                        {compact && <> - {highlighted(track.artist, q)}</>}
                                    </div>
                                    {!compact && (
                                        <div className="a">
                                            <span>{highlighted(track.artist, q)}</span>
                                            {!track.hasLyrics && <span className="tag">纯音乐</span>}
                                        </div>
                                    )}
                                </div>
                                <span className="d">
                                    <span className="dur">
                                        {track.durationMs === null ? "" : formatTime(track.durationMs)}
                                    </span>
                                    <button
                                        type="button"
                                        className="q-del"
                                        tabIndex={-1}
                                        aria-label="从播放队列移除"
                                        title="从播放队列移除"
                                        onPointerDown={event => event.stopPropagation()}
                                        onDoubleClick={event => event.stopPropagation()}
                                        onClick={event => {
                                            event.stopPropagation();
                                            onRemove(track.id);
                                        }}
                                    >
                                        <Icon name="close" />
                                    </button>
                                </span>
                            </div>
                        );
                    })
                )}
            </div>
        </aside>
    );
});
