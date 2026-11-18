import { useEffect, useRef, useState, type ReactNode } from "react";
import { Cover } from "../components/Cover";
import { LiveFill, LiveLyrics, LiveProgressBar, LiveTime } from "../components/Live";
import { type LyricLine } from "../components/LyricView";
import { MODES } from "../components/ModeMenu";
import { RemoveTrackDialog } from "../components/RemoveTrackDialog";
import { Toast } from "../components/Toast";
import { TunePanel } from "../components/TunePanel";
import { formatTime } from "../components/formatTime";
import { importSummary } from "../components/importSummary";
import {
    commands,
    listenPlaybackEvents,
    onBackButton,
    type ImportResult,
    type PlaybackEvent,
    type TrackView,
} from "../ipc";
import { usePlayerState } from "../store";
import { Snow } from "./Snow";
import "./android.css";

type View = "list" | "player" | "import";
type Sheet = "tune" | "list" | null;

function Icon({ path }: { path: string }) {
    return (
        <svg className="m-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d={path} />
        </svg>
    );
}

const icons = {
    play: "M8 5.5v13a1 1 0 0 0 1.5.86l10.5-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z",
    pause: "M6 5h4v14H6zM14 5h4v14h-4z",
    prev: "M5 5h2.2v14H5zM20 6.2v11.6a.8.8 0 0 1-1.2.7L10 12.7a.8.8 0 0 1 0-1.4l8.8-5.8a.8.8 0 0 1 1.2.7z",
    next: "M16.8 5H19v14h-2.2zM4 6.2v11.6a.8.8 0 0 0 1.2.7L14 12.7a.8.8 0 0 0 0-1.4L5.2 5.5A.8.8 0 0 0 4 6.2z",
    close: "M6 6l12 12M18 6 6 18",
    down: "M5 9l7 7 7-7",
    // 竖排两点“⋮”
    dots: "M12 7.5h.01M12 16.5h.01",
    queue: "M4 6h16M4 11h16M4 16h8M15.5 14l5 3-5 3z",
    plus: "M12 5v14M5 12h14",
    back: "M19 12H5M12 19l-7-7 7-7",
    // 音符的两个圆头用两段弧画出：Icon 只画一个 path
    note: "M9 18V6l11-2v12M4 18a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0M15 16a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0",
    folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
    search: "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM21 21l-4.35-4.35",
    check: "M4 12l5 5L20 6",
};

/** 安卓布局：首页即播放列表、播放页（模糊封面 + 飘雪 + 歌词）、导入页。 */
export function AndroidLayout() {
    const { state, notice, error } = usePlayerState();
    const [view, setView] = useState<View>("list");
    const [sheet, setSheet] = useState<Sheet>(null);
    const [query, setQuery] = useState("");
    const [lines, setLines] = useState<LyricLine[] | null>(null);
    const [preview, setPreview] = useState<number | null>(null);
    const [toast, setToast] = useState<{ message: string; icon?: ReactNode; key: number } | null>(null);
    const [importResult, setImportResult] = useState<ImportResult | null>(null);
    const [importBusy, setImportBusy] = useState(false);
    const [locate, setLocate] = useState<string | null>(null);
    const [removalId, setRemovalId] = useState<string | null>(null);
    useEffect(() => {
        if (view !== "list" || !locate) return;
        const row = document.querySelector<HTMLElement>(`.m-row[data-id="${CSS.escape(locate)}"]`);
        if (typeof row?.scrollIntoView === "function") row.scrollIntoView({ block: "center" });
        setLocate(null);
    }, [view, locate]);
    const toastKey = useRef(0);

    const playing = state?.playing ?? false;
    const current = state?.tracks.find((track: TrackView) => track.id === state.currentId);
    const pendingRemoval = state?.tracks.find(track => track.id === removalId);
    const mode = MODES.find(item => item.id === state?.mode) ?? MODES[1];
    const q = query.trim().toLowerCase();
    const visible = (state?.tracks ?? []).filter(
        track => !q || `${track.title}${track.artist}`.toLowerCase().includes(q),
    );
    const imported = Boolean(importResult && (importResult.added.length || importResult.lyricsAttached));
    const rateChanged = Math.abs((state?.rate ?? 1) - 1) > 0.01;

    const showToast = (message: string, icon?: ReactNode) => {
        toastKey.current += 1;
        setToast({ message, icon, key: toastKey.current });
    };
    const run = (promise: Promise<unknown>) => {
        void promise.catch(reason => showToast(String(reason)));
    };

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

    // 切换播放模式时在屏幕中间偏下显示深色轻提示
    const cycleMode = () => {
        if (!state) return;
        const ids = MODES.map(m => m.id);
        const next = ids[(ids.indexOf(state.mode) + 1) % ids.length];
        const label = MODES.find(m => m.id === next)?.label ?? "";
        run(commands.setMode(next).then(() => showToast(label)));
    };

    // 原生事件：焦点/耳机拔出 → 暂停；通知栏按钮 → 外壳命令
    useEffect(() => {
        let live = true;
        let unlisten: (() => void) | undefined;
        void commands.startPlaybackEvents().catch(() => {});
        void listenPlaybackEvents((event: PlaybackEvent) => {
            if (!live) return;
            if (event.kind === "pause") {
                run(commands.pause());
            } else if (event.action === "toggle") {
                run(commands.toggle());
            } else if (event.action === "next") {
                run(commands.next());
            } else {
                run(commands.prev());
            }
        })
            .then(dispose => {
                if (live) unlisten = dispose;
                else dispose();
            })
            .catch(() => {}); // 无 Tauri 环境（组件测试）忽略
        return () => {
            live = false;
            unlisten?.();
        };
    }, []);

    // 系统返回键在应用内逐级返回：关弹层 → 回列表 → 清空搜索 → 退到后台（不关闭应用，音乐继续）
    const back = useRef(() => {});
    back.current = () => {
        if (pendingRemoval) setRemovalId(null);
        else if (sheet) setSheet(null);
        else if (view !== "list") setView("list");
        else if (query) setQuery("");
        else run(commands.moveToBackground());
    };
    useEffect(() => {
        let live = true;
        let remove: (() => void) | undefined;
        void onBackButton(() => back.current())
            .then(dispose => {
                if (live) remove = dispose;
                else dispose();
            })
            .catch(() => {}); // 非安卓或无 Tauri 环境（组件测试）忽略
        return () => {
            live = false;
            remove?.();
        };
    }, []);

    // 通知栏/锁屏卡片同步：state 只在播放、暂停、跳转、换歌等真实变化时更新，进度由系统按倍速推算
    useEffect(() => {
        if (!current || !state) return;
        run(
            commands.notifyState({
                playing,
                title: current.title,
                artist: current.artist,
                durationMs: state.durationMs,
                positionMs: Math.round(state.positionMs),
                rate: state.rate,
            }),
        );
    }, [current, playing, state?.positionMs, state?.durationMs, state?.rate]);

    useEffect(() => {
        if (notice) showToast(`无法读取 ${notice.title}，已跳过`, <Icon path={icons.check} />);
    }, [notice]);
    useEffect(() => {
        if (error) showToast(error);
    }, [error]);

    // 选择与导入都在外壳完成：系统文件选择器提供文件名，歌名和歌词配对才正确；取消时返回 null
    const importWith = async (kind: "files" | "folder") => {
        setImportBusy(true);
        try {
            const result = await commands.pickAndImport(kind);
            if (result) setImportResult(result);
        } catch (reason) {
            showToast(String(reason));
        } finally {
            setImportBusy(false);
        }
    };
    // 去听歌：回到播放列表并滚到第一首导入的歌，不自动播放
    const goListen = () => {
        setLocate(importResult?.added[0] ?? null);
        setImportResult(null);
        setView("list");
    };

    return (
        <main data-platform="android" className={`android${view === "player" ? " android--player" : ""}`}>
            {view === "list" && (
                <>
                    <div className="lib-top">
                        <label className="m-search">
                            <Icon path={icons.search} />
                            <input
                                placeholder="搜索歌名或歌手"
                                aria-label="搜索歌名或歌手"
                                value={query}
                                onChange={event => setQuery(event.target.value)}
                            />
                        </label>
                        <button
                            type="button"
                            className="top-btn"
                            aria-label="导入音乐和歌词"
                            title="导入音乐和歌词"
                            onClick={() => {
                                setImportResult(null);
                                setView("import");
                            }}
                        >
                            <Icon path={icons.plus} />
                        </button>
                    </div>
                    <div className="m-list" role="list">
                        {visible.length === 0 ? (
                            <p className="m-empty">
                                {(state?.tracks ?? []).length
                                    ? "没有找到匹配的歌曲。"
                                    : "播放列表是空的。点右上角 + 导入音乐和歌词。"}
                            </p>
                        ) : (
                            visible.map(track => {
                                const on = track.id === state?.currentId;
                                return (
                                    <div
                                        key={track.id}
                                        role="listitem"
                                        data-id={track.id}
                                        className={`m-row${on ? " on" : ""}`}
                                    >
                                        <button
                                            type="button"
                                            className="m-play"
                                            onClick={() => {
                                                run(commands.playTrack(track.id));
                                                setView("player");
                                            }}
                                        >
                                            <span className="cv">
                                                <Cover
                                                    id={track.id}
                                                    title={track.title}
                                                    coverUrl={track.coverUrl}
                                                    size={46}
                                                />
                                                {on && (
                                                    <span className="eq" aria-hidden="true">
                                                        <i />
                                                        <i />
                                                        <i />
                                                        <i />
                                                    </span>
                                                )}
                                            </span>
                                            <span className="tx">
                                                <span className="t">{track.title}</span>
                                                <span className="a">
                                                    {!track.hasLyrics && <span className="m-tag">纯音乐</span>}
                                                    <span>{track.artist}</span>
                                                </span>
                                            </span>
                                        </button>
                                        <button
                                            type="button"
                                            className="m-del"
                                            aria-label={`从播放列表移除：${track.title}`}
                                            title="从播放列表移除"
                                            onClick={() => setRemovalId(track.id)}
                                        >
                                            <Icon path={icons.close} />
                                        </button>
                                    </div>
                                );
                            })
                        )}
                    </div>
                    <div className="mini">
                        {current && (
                            <Cover id={current.id} title={current.title} coverUrl={current.coverUrl} size={42} />
                        )}
                        <button type="button" className="tx" onClick={() => current && setView("player")}>
                            <span className="t">{current?.title ?? "还没有播放"}</span>
                            <span className="a">{current?.artist ?? "点一首歌开始"}</span>
                        </button>
                        <button
                            type="button"
                            className="mini-btn"
                            aria-label={playing ? "暂停" : "播放"}
                            onClick={() => current && run(commands.toggle())}
                        >
                            <Icon path={playing ? icons.pause : icons.play} />
                        </button>
                        <button
                            type="button"
                            className="mini-btn"
                            aria-label="下一首"
                            onClick={() => current && run(commands.next())}
                        >
                            <Icon path={icons.next} />
                        </button>
                        <LiveFill className="p" preview={preview} durationMs={state?.durationMs ?? null} />
                    </div>
                    <Toast
                        message={toast?.message ?? null}
                        variant="mobile"
                        icon={toast?.icon}
                        id={toast?.key}
                        onDismiss={() => setToast(null)}
                    />
                </>
            )}

            {view === "player" && (
                <>
                    {current?.coverUrl && (
                        <div className="np-bg" style={{ backgroundImage: `url("${current.coverUrl}")` }} />
                    )}
                    <div className="np-shade" aria-hidden="true" />
                    <Snow className="np-snow" />
                    <div className="np-top">
                        <button type="button" aria-label="收起" onClick={() => setView("list")}>
                            <Icon path={icons.down} />
                        </button>
                        <i />
                    </div>
                    <div className="np-head">
                        <h2 className="stage-title">{current?.title}</h2>
                        <p className="stage-artist">{current?.artist}</p>
                    </div>
                    <div className="np-lyrics">
                        <LiveLyrics lines={lines ?? []} preview={preview} onSeek={ms => run(commands.seek(ms))} />
                        {lines !== null && lines.length === 0 && (
                            <p className="ly is-active instrumental">纯音乐，请欣赏</p>
                        )}
                    </div>
                    <div className="np-ctl">
                        <div className="np-tools">
                            <button
                                type="button"
                                className={`np-rate${rateChanged || (state?.pitch ?? 0) !== 0 ? " changed" : ""}`}
                                aria-label="升降调和倍速"
                                title="升降调和倍速"
                                onClick={() => setSheet("tune")}
                            >
                                <Icon path={icons.dots} />
                                <sup>{rateChanged ? `${state?.rate.toFixed(1)}X` : ""}</sup>
                            </button>
                        </div>
                        <LiveProgressBar
                            variant="mobile"
                            durationMs={state?.durationMs ?? null}
                            onPreviewChange={setPreview}
                            onSeek={ms => run(commands.seek(ms))}
                        />
                        <div className="np-times">
                            <span>
                                <LiveTime preview={preview} />
                            </span>
                            <span>{formatTime(state?.durationMs)}</span>
                        </div>
                        <div className="np-btns">
                            <button
                                type="button"
                                className="side"
                                aria-label={`播放模式：${mode.label}`}
                                onClick={cycleMode}
                            >
                                <Icon path={mode.path} />
                            </button>
                            <button type="button" aria-label="上一首" onClick={() => current && run(commands.prev())}>
                                <Icon path={icons.prev} />
                            </button>
                            <button
                                type="button"
                                className="big"
                                aria-label={playing ? "暂停" : "播放"}
                                onClick={() =>
                                    current
                                        ? run(commands.toggle())
                                        : visible[0] && run(commands.playTrack(visible[0].id))
                                }
                            >
                                <Icon path={playing ? icons.pause : icons.play} />
                            </button>
                            <button type="button" aria-label="下一首" onClick={() => current && run(commands.next())}>
                                <Icon path={icons.next} />
                            </button>
                            <button
                                type="button"
                                className="side"
                                aria-label="播放列表"
                                onClick={() => setSheet("list")}
                            >
                                <Icon path={icons.queue} />
                            </button>
                        </div>
                    </div>
                    {sheet && (
                        <>
                            <div className="scrim" data-testid="scrim" onClick={() => setSheet(null)} />
                            {sheet === "tune" ? (
                                <div className="sheet tune-sheet">
                                    <div className="grab" />
                                    <TunePanel
                                        rate={state?.rate ?? 1}
                                        pitch={state?.pitch ?? 0}
                                        variant="mobile"
                                        onRateChange={value => run(commands.setRate(value))}
                                        onPitchChange={value => run(commands.setPitch(value))}
                                    />
                                </div>
                            ) : (
                                <div className="sheet list-sheet" role="dialog" aria-label="播放列表">
                                    <div className="grab" />
                                    <div className="ls-head">
                                        <b>播放列表</b>
                                        <span> ({(state?.tracks ?? []).length})</span>
                                    </div>
                                    <div className="ls-list">
                                        {(state?.tracks ?? []).map(track => {
                                            const on = track.id === state?.currentId;
                                            return (
                                                <div key={track.id} className={`ls-row${on ? " on" : ""}`}>
                                                    <button
                                                        type="button"
                                                        className="ls-play"
                                                        onClick={() => run(commands.playTrack(track.id))}
                                                    >
                                                        <span className="ls-cv">
                                                            <Cover
                                                                id={track.id}
                                                                title={track.title}
                                                                coverUrl={track.coverUrl}
                                                                size={48}
                                                            />
                                                            {on && (
                                                                <span className="ls-eq" aria-hidden="true">
                                                                    <i />
                                                                    <i />
                                                                    <i />
                                                                    <i />
                                                                </span>
                                                            )}
                                                        </span>
                                                        <span className="ls-tx">
                                                            <span className="t">{track.title}</span>
                                                            <span className="a">
                                                                {!track.hasLyrics && (
                                                                    <span className="m-tag">纯音乐</span>
                                                                )}
                                                                <span>{track.artist}</span>
                                                            </span>
                                                        </span>
                                                    </button>
                                                    <button
                                                        type="button"
                                                        className="ls-del"
                                                        aria-label={`从播放列表移除：${track.title}`}
                                                        onClick={() => setRemovalId(track.id)}
                                                    >
                                                        <Icon path={icons.close} />
                                                    </button>
                                                </div>
                                            );
                                        })}
                                    </div>
                                </div>
                            )}
                        </>
                    )}
                    <Toast
                        message={toast?.message ?? null}
                        variant="mobile"
                        icon={toast?.icon}
                        id={toast?.key}
                        onDismiss={() => setToast(null)}
                    />
                </>
            )}

            {view === "import" && (
                <>
                    <div className="imp-top">
                        <button type="button" className="top-btn" aria-label="返回" onClick={() => setView("list")}>
                            <Icon path={icons.back} />
                        </button>
                    </div>
                    <div className="imp">
                        <div className="imp-hero" aria-hidden="true">
                            <Icon path={icons.note} />
                        </div>
                        <h2>导入音乐</h2>
                        <button
                            type="button"
                            className={`imp-btn${imported ? "" : " primary"}`}
                            disabled={importBusy}
                            onClick={() => void importWith("files")}
                        >
                            <Icon path={icons.note} />
                            选择歌曲
                        </button>
                        <button
                            type="button"
                            className="imp-btn"
                            disabled={importBusy}
                            onClick={() => void importWith("folder")}
                        >
                            <Icon path={icons.folder} />
                            导入整个文件夹
                        </button>
                        {importResult && (
                            <div className="imp-done" role="status">
                                <p>{importSummary(importResult)}</p>
                                {imported && (
                                    <button type="button" className="imp-btn primary" onClick={goListen}>
                                        去听歌
                                    </button>
                                )}
                            </div>
                        )}
                    </div>
                </>
            )}
            {pendingRemoval && (
                <RemoveTrackDialog
                    track={pendingRemoval}
                    onCancel={() => setRemovalId(null)}
                    onConfirm={() => {
                        setRemovalId(null);
                        run(
                            commands.removeTrack(pendingRemoval.id).then(() => {
                                showToast("已从播放列表移除", <Icon path={icons.check} />);
                            }),
                        );
                    }}
                />
            )}
        </main>
    );
}
