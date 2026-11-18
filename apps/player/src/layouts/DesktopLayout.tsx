import { useCallback, useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Cover } from "../components/Cover";
import { LiveLyrics, LiveProgressBar, LiveTime } from "../components/Live";
import { type LyricLine } from "../components/LyricView";
import { MODES, ModeMenu } from "../components/ModeMenu";
import { RemoveTrackDialog } from "../components/RemoveTrackDialog";
import { Toast } from "../components/Toast";
import { TunePanel } from "../components/TunePanel";
import { formatTime } from "../components/formatTime";
import { commands, type TrackView } from "../ipc";
import { playerStore, usePlayerState } from "../store";
import { ImportDialog } from "./ImportDialog";
import { QueuePanel } from "./QueuePanel";
import { useNativeImportDrop } from "./useNativeImportDrop";
import "./DesktopLayout.css";

type Popup = "mode" | "tune" | "volume" | null;
const NO_TRACKS: TrackView[] = [];
type Colors = { top: string; bottom: string; accent: string; image: string | null };
const glazes = ["#2440b8", "#8fae9a", "#1c2629", "#9b3b4a", "#b08a3e", "#d6e2ea", "#4e6b5c"];
const icons = {
    play: "M8 5.5v13a1 1 0 0 0 1.5.86l10.5-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z",
    pause: "M6 5h4v14H6zM14 5h4v14h-4z",
    prev: "M5 5h2.2v14H5zM20 6.2v11.6a.8.8 0 0 1-1.2.7L10 12.7a.8.8 0 0 1 0-1.4l8.8-5.8a.8.8 0 0 1 1.2.7z",
    next: "M16.8 5H19v14h-2.2zM4 6.2v11.6a.8.8 0 0 0 1.2.7L14 12.7a.8.8 0 0 0 0-1.4L5.2 5.5A.8.8 0 0 0 4 6.2z",
    // 竖排两点（⋮），与手机播放页的升降调/倍速按钮一致
    dots: "M12 7.5h.01M12 16.5h.01",
    volume: "M4 9v6h4l5 4V5L8 9H4zM16.5 8.5a5 5 0 0 1 0 7",
    mute: "M4 9v6h4l5 4V5L8 9H4zM16.5 9.5l5 5M21.5 9.5l-5 5",
    queue: "M4 6h16M4 11h16M4 16h8M15.5 14l5 3-5 3z",
    min: "M6 12h12",
    max: "M6 6h12v12H6z",
    close: "M6 6l12 12M18 6 6 18",
};
function Icon({ path, filled = false }: { path: string; filled?: boolean }) {
    return (
        <svg className={`desktop-icon${filled ? " desktop-icon--filled" : ""}`} viewBox="0 0 24 24" aria-hidden="true">
            <path d={path} />
        </svg>
    );
}
function fallback(id?: string): number[] {
    if (!id) return [138, 111, 208];
    let hash = 2166136261;
    for (const char of id) {
        hash ^= char.codePointAt(0)!;
        hash = Math.imul(hash, 16777619);
    }
    const hex = glazes[(hash >>> 0) % glazes.length];
    return [1, 3, 5].map(index => parseInt(hex.slice(index, index + 2), 16));
}
function mixed(base: number[], factor: number, target: number[]) {
    return `rgb(${base.map((value, index) => Math.round(value + (target[index] - value) * factor)).join(",")})`;
}
function colors(base: number[], image: string | null, fromCover: boolean): Colors {
    return {
        top: mixed(base, fromCover ? 0.6 : 0.62, fromCover ? [12, 10, 16] : [10, 12, 18]),
        bottom: mixed(base, fromCover ? 0.88 : 0.86, fromCover ? [12, 10, 16] : [7, 8, 12]),
        accent: mixed(base, 0.4, [255, 255, 255]),
        image,
    };
}
function withRevision(url: string, revision: number | undefined) {
    return revision === undefined ? url : `${url}${url.includes("?") ? "&" : "?"}coverRevision=${revision}`;
}

function useStageColors(track: TrackView | undefined, revision?: number) {
    const [result, setResult] = useState<Colors>(() => colors(fallback(track?.id), null, false));
    useEffect(() => {
        let active = true;
        setResult(colors(fallback(track?.id), null, false));
        if (!track?.coverUrl)
            return () => {
                active = false;
            };
        const src = withRevision(track.coverUrl, revision);
        const image = new Image();
        image.crossOrigin = "anonymous";
        image.onload = () => {
            if (!active) return;
            try {
                const canvas = document.createElement("canvas");
                canvas.width = canvas.height = 12;
                const context = canvas.getContext("2d");
                if (!context) return;
                context.drawImage(image, 0, 0, 12, 12);
                const pixels = context.getImageData(0, 0, 12, 12).data;
                const base = [0, 0, 0];
                for (let i = 0; i < pixels.length; i += 4) {
                    base[0] += pixels[i];
                    base[1] += pixels[i + 1];
                    base[2] += pixels[i + 2];
                }
                setResult(
                    colors(
                        base.map(value => value / (pixels.length / 4)),
                        src,
                        true,
                    ),
                );
            } catch {
                /* 无法读取像素时继续使用字封色。 */
            }
        };
        image.src = src;
        return () => {
            active = false;
        };
    }, [track?.id, track?.coverUrl, revision]);
    return result;
}

export function DesktopLayout() {
    const { state, notice, error } = usePlayerState();
    const [queueOpen, setQueueOpen] = useState(true);
    const [popup, setPopup] = useState<Popup>(null);
    const [importOpen, setImportOpen] = useState(false);
    const [removalId, setRemovalId] = useState<string | null>(null);
    const [preview, setPreview] = useState<number | null>(null);
    const [lines, setLines] = useState<LyricLine[]>([]);
    const [desktopLyrics, setDesktopLyrics] = useState(false);
    const [toast, setToast] = useState<{ message: string; key: number } | null>(null);
    const [revisions, setRevisions] = useState<Record<string, number>>({});
    const [locateRequest, setLocateRequest] = useState<{ id: string; version: number }>();
    const [dragging, setDragging] = useState(false);
    const [droppedPaths, setDroppedPaths] = useState<string[] | null>(null);
    const slider = useRef<HTMLDivElement>(null);
    const current = state?.tracks.find(track => track.id === state.currentId);
    const pendingRemoval = state?.tracks.find(track => track.id === removalId);
    const revision = current ? revisions[current.id] : undefined;
    const stage = useStageColors(current, revision);
    const mode = MODES.find(item => item.id === state?.mode) ?? MODES[1];
    const volume = state?.volume ?? 0.8;
    const rate = state?.rate ?? 1;
    const pitch = state?.pitch ?? 0;
    const style = {
        "--desktop-top": stage.top,
        "--desktop-bottom": stage.bottom,
        "--accent": stage.accent,
    } as CSSProperties;
    // 轻提示只显示最近发生的一件事：移除、操作失败、文件无法读取或后端错误
    const toastKey = useRef(0);
    const showToast = useCallback((message: string) => setToast({ message, key: ++toastKey.current }), []);
    const run = useCallback(
        (promise: Promise<unknown>) => {
            void promise.catch(reason => showToast(String(reason)));
        },
        [showToast],
    );
    // 播放队列是 memo 组件：回调保持稳定，进度预览等局部更新不会重渲染整张列表
    const playTrack = useCallback((id: string) => run(commands.playTrack(id)), [run]);
    const removeTrack = useCallback(
        (id: string) => run(commands.removeTrack(id).then(() => showToast("已从播放列表移除"))),
        [run, showToast],
    );
    const moveTrack = useCallback((from: number, to: number) => run(commands.moveTrack(from, to)), [run]);
    const openImport = useCallback(() => setImportOpen(true), []);
    const closeQueue = useCallback(() => setQueueOpen(false), []);
    useEffect(() => {
        if (notice) showToast(`无法读取 ${notice.title}，已跳过`);
    }, [notice]);
    useEffect(() => {
        if (error) showToast(error);
    }, [error]);

    useEffect(() => {
        let live = true;
        setLines([]);
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
    }, [current?.id, current?.hasLyrics, revision]);
    useEffect(() => {
        const keydown = (event: KeyboardEvent) => {
            if (pendingRemoval) return;
            if (event.key === "Escape") {
                setPopup(null);
                setImportOpen(false);
                return;
            }
            const target = event.target instanceof Element ? event.target : null;
            if (
                event.code === "Space" &&
                current &&
                !importOpen &&
                !target?.closest('input, button, textarea, select, [role="slider"], [contenteditable="true"]')
            ) {
                event.preventDefault();
                run(commands.toggle());
            }
        };
        const outside = (event: MouseEvent) => {
            const target = event.target instanceof Element ? event.target : null;
            if (!target?.closest(".desktop-pop, [data-pop-trigger]")) setPopup(null);
        };
        document.addEventListener("keydown", keydown);
        document.addEventListener("click", outside);
        return () => {
            document.removeEventListener("keydown", keydown);
            document.removeEventListener("click", outside);
        };
    }, [current?.id, importOpen, pendingRemoval]);
    useEffect(() => {
        if (popup === "volume") slider.current?.focus();
    }, [popup]);
    // 弹层相对触发按钮居中；超出底栏时向内收回，与底栏两端保持 8px
    useLayoutEffect(() => {
        if (!popup) return;
        const clamp = () => {
            const pop = document.querySelector<HTMLElement>(".desktop-pop");
            const bar = document.querySelector<HTMLElement>(".desktop-bar");
            if (!pop || !bar) return;
            pop.style.setProperty("--pop-shift", "0px");
            const p = pop.getBoundingClientRect(),
                b = bar.getBoundingClientRect();
            const shift = p.left < b.left + 8 ? b.left + 8 - p.left : p.right > b.right - 8 ? b.right - 8 - p.right : 0;
            pop.style.setProperty("--pop-shift", `${Math.round(shift)}px`);
        };
        clamp();
        window.addEventListener("resize", clamp);
        return () => window.removeEventListener("resize", clamp);
    }, [popup]);
    useNativeImportDrop({
        onEnter: () => {
            setImportOpen(true);
            setDragging(true);
        },
        onLeave: () => setDragging(false),
        onDrop: paths => {
            setImportOpen(true);
            setDragging(false);
            setDroppedPaths(paths);
        },
    });
    const toggle = (next: Popup) => setPopup(previous => (previous === next ? null : next));
    const setVolume = (value: number) => run(commands.setVolume(Math.max(0, Math.min(1, value))));
    const volumeAt = (element: HTMLElement, clientY: number) => {
        const rect = element.getBoundingClientRect();
        if (rect.height > 0) setVolume((rect.bottom - clientY) / rect.height);
    };
    const windowAction = (action: "minimize" | "toggleMaximize" | "close") => run(getCurrentWindow()[action]());
    const button = (
        label: string,
        path: string,
        action: () => void,
        className: string,
        filled = false,
        extra?: ReactNode,
    ) => (
        <button type="button" className={className} aria-label={label} title={label} onClick={action}>
            <Icon path={path} filled={filled} />
            {extra}
        </button>
    );

    return (
        <main data-platform="desktop" className={`desktop${queueOpen ? "" : " desktop--queue-closed"}`} style={style}>
            <div
                className={`desktop-background${stage.image ? " is-visible" : ""}`}
                style={stage.image ? { backgroundImage: `url("${stage.image}")` } : undefined}
            />
            <div className="desktop-glow" />
            <section className="desktop-stage">
                <div className="desktop-topbar" data-tauri-drag-region>
                    {button("最小化", icons.min, () => windowAction("minimize"), "desktop-window-button")}
                    {button("最大化", icons.max, () => windowAction("toggleMaximize"), "desktop-window-button")}
                    {button("关闭", icons.close, () => windowAction("close"), "desktop-window-button")}
                </div>
                <div className="desktop-now-playing">
                    <div className="desktop-heading">
                        {current && (
                            <Cover
                                id={current.id}
                                title={current.title}
                                coverUrl={current.coverUrl}
                                revision={revision}
                                size={64}
                            />
                        )}
                        <div className="desktop-heading-text">
                            <h1>{current?.title ?? "选一首歌开始"}</h1>
                            <p>{current?.artist ?? "在右侧播放队列里双击歌曲"}</p>
                        </div>
                    </div>
                    {current && (
                        <LiveLyrics
                            lines={lines}
                            preview={preview}
                            onSeek={ms => run(commands.seek(ms))}
                            className={queueOpen ? "" : "center"}
                        />
                    )}
                </div>
                <QueuePanel
                    tracks={state?.tracks ?? NO_TRACKS}
                    currentId={state?.currentId ?? null}
                    playing={state?.playing ?? false}
                    open={queueOpen}
                    coverRevisions={revisions}
                    locateRequest={locateRequest}
                    onPlay={playTrack}
                    onRemove={setRemovalId}
                    onMove={moveTrack}
                    onImport={openImport}
                    onClose={closeQueue}
                />
            </section>
            <footer className="desktop-bar">
                <div className="desktop-progress">
                    <LiveProgressBar
                        key={current?.id ?? "empty"}
                        durationMs={state?.durationMs ?? null}
                        onPreviewChange={setPreview}
                        onSeek={ms => run(commands.seek(ms))}
                    />
                </div>
                <div className="desktop-now">
                    {current && (
                        <Cover
                            id={current.id}
                            title={current.title}
                            coverUrl={current.coverUrl}
                            revision={revision}
                            size={44}
                        />
                    )}
                    <div className="desktop-now-text">
                        <div title={current?.title}>{current?.title ?? "还没有播放"}</div>
                        <p title={current?.artist}>{current?.artist ?? ""}</p>
                    </div>
                    <span className="desktop-time">
                        <span>
                            <LiveTime preview={preview} />
                        </span>
                        <span className="desktop-time-separator">/</span>
                        <span>{formatTime(state?.durationMs)}</span>
                    </span>
                </div>
                <div className="desktop-controls">
                    <div className="desktop-control-wrap">
                        <button
                            type="button"
                            data-pop-trigger
                            className="desktop-control-side"
                            aria-label={`播放模式：${mode.label}`}
                            title={mode.label}
                            aria-haspopup="menu"
                            aria-expanded={popup === "mode"}
                            onClick={() => toggle("mode")}
                        >
                            <Icon path={mode.path} />
                        </button>
                        {popup === "mode" && (
                            <div className="desktop-pop">
                                <ModeMenu
                                    mode={state?.mode ?? "loop"}
                                    onSelect={value => {
                                        run(commands.setMode(value));
                                        setPopup(null);
                                    }}
                                    onClose={() => setPopup(null)}
                                />
                            </div>
                        )}
                    </div>
                    {button("上一首", icons.prev, () => run(commands.prev()), "desktop-control-skip", true)}
                    {button(
                        state?.playing ? "暂停" : "播放",
                        state?.playing ? icons.pause : icons.play,
                        () => run(commands.toggle()),
                        "desktop-control-play",
                        true,
                    )}
                    {button("下一首", icons.next, () => run(commands.next()), "desktop-control-skip", true)}
                    <div className="desktop-control-wrap">
                        <button
                            type="button"
                            data-pop-trigger
                            className="desktop-control-side desktop-tune"
                            aria-label="升降调和倍速"
                            title="升降调和倍速"
                            aria-haspopup="dialog"
                            aria-expanded={popup === "tune"}
                            aria-pressed={rate !== 1 || pitch !== 0}
                            onClick={() => toggle("tune")}
                        >
                            <Icon path={icons.dots} />
                            <sup>{rate !== 1 ? `${rate.toFixed(1)}X` : ""}</sup>
                        </button>
                        {popup === "tune" && (
                            <div className="desktop-pop" role="dialog" aria-label="升降调和倍速">
                                <TunePanel
                                    rate={rate}
                                    pitch={pitch}
                                    onRateChange={value => run(commands.setRate(value))}
                                    onPitchChange={value => run(commands.setPitch(value))}
                                />
                            </div>
                        )}
                    </div>
                </div>
                <div className="desktop-right">
                    <button
                        type="button"
                        className="desktop-side-button desktop-lyrics-button"
                        aria-label="桌面歌词"
                        aria-pressed={desktopLyrics}
                        title="桌面歌词"
                        onClick={() => {
                            void commands
                                .toggleDesktopLyrics()
                                .then(setDesktopLyrics)
                                .catch(reason => showToast(String(reason)));
                        }}
                    >
                        词
                    </button>
                    <button
                        type="button"
                        className="desktop-side-button"
                        aria-label="播放队列"
                        aria-pressed={queueOpen}
                        title="播放队列"
                        onClick={() => setQueueOpen(open => !open)}
                    >
                        <Icon path={icons.queue} />
                    </button>
                    <div className="desktop-control-wrap">
                        <button
                            type="button"
                            data-pop-trigger
                            className="desktop-side-button"
                            aria-label="音量"
                            title="音量"
                            aria-haspopup="dialog"
                            aria-expanded={popup === "volume"}
                            onClick={() => toggle("volume")}
                        >
                            <Icon path={volume === 0 ? icons.mute : icons.volume} />
                        </button>
                        {popup === "volume" && (
                            <div className="desktop-pop desktop-volume" role="dialog" aria-label="调节音量">
                                <div
                                    ref={slider}
                                    className="desktop-volume-slider"
                                    role="slider"
                                    tabIndex={0}
                                    aria-label="音量"
                                    aria-valuemin={0}
                                    aria-valuemax={100}
                                    aria-valuenow={Math.round(volume * 100)}
                                    aria-orientation="vertical"
                                    onPointerDown={event => {
                                        event.currentTarget.setPointerCapture?.(event.pointerId);
                                        volumeAt(event.currentTarget, event.clientY);
                                    }}
                                    onPointerMove={event => {
                                        if (event.buttons) volumeAt(event.currentTarget, event.clientY);
                                    }}
                                    onKeyDown={event => {
                                        const step =
                                            event.key === "ArrowUp" || event.key === "ArrowRight"
                                                ? 0.05
                                                : event.key === "ArrowDown" || event.key === "ArrowLeft"
                                                  ? -0.05
                                                  : 0;
                                        if (step) {
                                            event.preventDefault();
                                            setVolume(Math.round((volume + step) * 100) / 100);
                                        }
                                    }}
                                >
                                    <span className="desktop-volume-fill" style={{ height: `${volume * 100}%` }} />
                                    <span className="desktop-volume-thumb" style={{ bottom: `${volume * 100}%` }} />
                                </div>
                                <span className="desktop-volume-value">{Math.round(volume * 100)}%</span>
                            </div>
                        )}
                    </div>
                </div>
            </footer>
            {importOpen && (
                <ImportDialog
                    onClose={() => {
                        setImportOpen(false);
                        setDragging(false);
                        setDroppedPaths(null);
                    }}
                    droppedPaths={droppedPaths}
                    dragging={dragging}
                    onImported={ids =>
                        setRevisions(previous => {
                            const next = { ...previous };
                            for (const id of ids) next[id] = (next[id] ?? 0) + 1;
                            return next;
                        })
                    }
                    onGoListen={id => {
                        if (id) {
                            setQueueOpen(true);
                            setLocateRequest(previous => ({ id, version: (previous?.version ?? 0) + 1 }));
                        }
                    }}
                />
            )}
            {pendingRemoval && (
                <RemoveTrackDialog
                    track={pendingRemoval}
                    onCancel={() => setRemovalId(null)}
                    onConfirm={() => {
                        setRemovalId(null);
                        removeTrack(pendingRemoval.id);
                    }}
                />
            )}
            <Toast
                message={toast?.message ?? null}
                id={toast?.key}
                onDismiss={() => {
                    setToast(null);
                    playerStore.dismissNotice();
                }}
            />
        </main>
    );
}
