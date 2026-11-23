import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { NameDialog } from "../components/NameDialog";
import { RemoveTrackDialog } from "../components/RemoveTrackDialog";
import { Toast } from "../components/Toast";
import { commands, listenPlaybackEvents, onBackButton, type PlaybackEvent, type TrackView } from "../ipc";
import { useLibrary, usePlayerState } from "../store";
import { AddSongsPage } from "./android/AddSongsPage";
import { HomePage } from "./android/HomePage";
import { ImportPage } from "./android/ImportPage";
import { MiniPlayer } from "./android/MiniPlayer";
import { PlaylistPage } from "./android/PlaylistPage";
import { PlayerPage, type PlayerSheet } from "./android/PlayerPage";
import { ALL_MUSIC, Icon, PAGE_MS, icons } from "./android/shared";
import "./android.css";

type Page =
    | { kind: "home" }
    | { kind: "playlist"; id: string }
    | { kind: "add"; id: string }
    | { kind: "import" }
    | { kind: "player" };
type Entry = { key: number; page: Page; leaving: boolean };
type Sheet = PlayerSheet | "playlist-menu" | null;
type Dialog =
    | { type: "new-playlist" }
    | { type: "rename-playlist"; id: string }
    | { type: "delete-playlist"; id: string }
    | { type: "remove-track"; track: TrackView }
    | null;
type ToastState = { message: string; done: boolean; key: number };

const NO_TRACKS: TrackView[] = [];
const NO_FLASH: ReadonlySet<string> = new Set();

const samePage = (a: Page, b: Page) => a.kind === b.kind && ("id" in a && "id" in b ? a.id === b.id : true);
/** 栈顶：最后一个没有在退出的页面。 */
const topOf = (entries: Entry[]) => entries.reduce<Entry>((top, entry) => (entry.leaving ? top : entry), entries[0]);

/**
 * 安卓布局：页面栈（首页歌单列表 → 歌单 → 选歌或导入 → 播放页）、底部弹层和对话框。
 * 首页的「全部音乐」就是曲库；从歌单点播时播放列表整份换成这个歌单的顺序。
 */
export function AndroidLayout() {
    const { state, notice, error } = usePlayerState();
    const library = useLibrary();
    const [stack, setStack] = useState<Entry[]>(() => [{ key: 0, page: { kind: "home" }, leaving: false }]);
    // 搜索词按页面保存：系统返回键先清空它，再退出页面
    const [queries, setQueries] = useState<Record<number, string>>({});
    const [sheet, setSheet] = useState<Sheet>(null);
    const [dialog, setDialog] = useState<Dialog>(null);
    const [flash, setFlash] = useState<ReadonlySet<string>>(NO_FLASH);
    const [locate, setLocate] = useState<string | null>(null);
    const [toast, setToast] = useState<ToastState | null>(null);
    const nextKey = useRef(1);
    const leaveTimers = useRef(new Map<number, number>());

    const top = topOf(stack);
    const topKey = top.key;
    const currentId = state?.currentId ?? null;
    const current = state?.tracks.find(track => track.id === state.currentId);
    const byId = useMemo(
        () => new Map((library?.tracks ?? NO_TRACKS).map(track => [track.id, track] as const)),
        [library],
    );
    const describe = useCallback(
        (id: string) => {
            if (id === ALL_MUSIC.id) {
                return { name: ALL_MUSIC.name, builtIn: true, members: library?.tracks ?? NO_TRACKS };
            }
            const playlist = library?.playlists.find(item => item.id === id);
            return {
                name: playlist?.name ?? "",
                builtIn: false,
                members: (playlist?.trackIds ?? []).flatMap(trackId => byId.get(trackId) ?? []),
            };
        },
        [library, byId],
    );
    const lists = [
        { id: ALL_MUSIC.id, name: ALL_MUSIC.name, count: library?.tracks.length ?? 0, builtIn: true },
        ...(library?.playlists ?? []).map(playlist => ({
            id: playlist.id,
            name: playlist.name,
            count: playlist.trackIds.length,
        })),
    ];

    const toastKey = useRef(0);
    const showToast = useCallback((message: string, done = false) => {
        toastKey.current += 1;
        setToast({ message, done, key: toastKey.current });
    }, []);
    const run = useCallback(
        (promise: Promise<unknown>) => {
            void promise.catch(reason => showToast(String(reason)));
        },
        [showToast],
    );

    // 页面栈：同一页已在栈里时提到最上面，否则叠上去
    const push = useCallback((page: Page) => {
        const key = nextKey.current++;
        setStack(previous => {
            const existing = previous.find(entry => samePage(entry.page, page));
            return existing
                ? [...previous.filter(entry => entry !== existing), { ...existing, leaving: false }]
                : [...previous, { key, page, leaving: false }];
        });
    }, []);
    const pop = useCallback(() => {
        setStack(previous =>
            previous.map(entry =>
                entry === topOf(previous) && entry !== previous[0] ? { ...entry, leaving: true } : entry,
            ),
        );
    }, []);
    // 退出的页面滑出动画结束后移除；中途又被推回栈顶则取消
    useEffect(() => {
        const timers = leaveTimers.current;
        for (const entry of stack) {
            const timer = timers.get(entry.key);
            if (!entry.leaving) {
                if (timer !== undefined) {
                    clearTimeout(timer);
                    timers.delete(entry.key);
                }
            } else if (timer === undefined) {
                timers.set(
                    entry.key,
                    window.setTimeout(() => {
                        timers.delete(entry.key);
                        setStack(previous => previous.filter(item => item.key !== entry.key));
                        setQueries(previous => {
                            if (!(entry.key in previous)) return previous;
                            const next = { ...previous };
                            delete next[entry.key];
                            return next;
                        });
                    }, PAGE_MS + 20),
                );
            }
        }
    }, [stack]);
    useEffect(() => {
        const timers = leaveTimers.current;
        return () => {
            for (const timer of timers.values()) clearTimeout(timer);
            timers.clear();
        };
    }, []);
    // 弹层只属于打开它的那一页：栈顶变了就收起
    useEffect(() => {
        setSheet(null);
    }, [topKey]);
    // 播放列表被清空时没有可播放的歌：退出播放页
    const playerOnTop = top.page.kind === "player";
    useEffect(() => {
        if (playerOnTop && state && !currentId) pop();
    }, [playerOnTop, state, currentId, pop]);

    useEffect(() => {
        if (!flash.size) return;
        const timer = window.setTimeout(() => setFlash(NO_FLASH), 2400);
        return () => clearTimeout(timer);
    }, [flash]);

    const setQuery = useCallback((key: number, value: string) => {
        setQueries(previous => (previous[key] === value ? previous : { ...previous, [key]: value }));
    }, []);
    const closeDialog = useCallback(() => setDialog(null), []);

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
    }, [run]);

    // 系统返回键逐级返回：对话框 → 弹层 → 清空搜索 → 上一页 → 退到后台（不关闭应用，音乐继续）
    const back = useRef(() => {});
    back.current = () => {
        if (dialog) return setDialog(null);
        if (sheet) return setSheet(null);
        if (topKey >= 0 && queries[topKey]) return setQuery(topKey, "");
        if (stack.some(entry => !entry.leaving && entry !== stack[0])) return pop();
        run(commands.moveToBackground());
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
    const playing = state?.playing ?? false;
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
    }, [current, playing, state?.positionMs, state?.durationMs, state?.rate, run]);

    useEffect(() => {
        if (notice) showToast(`无法读取 ${notice.title}，已跳过`, true);
    }, [notice, showToast]);
    useEffect(() => {
        if (error) showToast(error);
    }, [error, showToast]);

    /** 对话框提交：失败时提示原因，对话框保持打开 */
    const submit = async (action: () => Promise<void>) => {
        try {
            await action();
        } catch (reason) {
            showToast(String(reason));
        }
    };
    const playFrom = (playlistId: string | null, filteredIds: string[] | null, start: string) => {
        // 点的还是正在听的那首：不重新开始，暂停着就继续播
        if (start === currentId) {
            if (state && !state.playing) run(commands.play());
        } else {
            run(filteredIds ? commands.playFrom(filteredIds, start) : commands.playPlaylist(playlistId, start));
        }
        push({ kind: "player" });
    };
    const openPlayer = () => push({ kind: "player" });
    const clearLocate = useCallback(() => setLocate(null), []);

    let dialogElement = null;
    if (dialog?.type === "new-playlist") {
        dialogElement = (
            <NameDialog
                title="新建歌单"
                okLabel="创建"
                onCancel={closeDialog}
                onSubmit={value =>
                    submit(async () => {
                        const id = await commands.createPlaylist(value);
                        closeDialog();
                        push({ kind: "playlist", id });
                    })
                }
            />
        );
    } else if (dialog?.type === "rename-playlist") {
        const { id } = dialog;
        dialogElement = (
            <NameDialog
                key={id}
                title="重命名歌单"
                value={describe(id).name}
                okLabel="保存"
                onCancel={closeDialog}
                onSubmit={value =>
                    submit(async () => {
                        await commands.renamePlaylist(id, value);
                        closeDialog();
                    })
                }
            />
        );
    } else if (dialog?.type === "delete-playlist") {
        const { id } = dialog;
        const name = describe(id).name;
        dialogElement = (
            <ConfirmDialog
                title="删除歌单"
                okLabel="删除"
                onCancel={closeDialog}
                description={`确定删除「${name}」？歌单里的音乐不会被删除，仍保留在「${ALL_MUSIC.name}」。`}
                onConfirm={() =>
                    submit(async () => {
                        await commands.deletePlaylist(id);
                        closeDialog();
                        pop();
                        showToast(`已删除歌单「${name}」`, true);
                    })
                }
            />
        );
    } else if (dialog?.type === "remove-track") {
        const { track } = dialog;
        dialogElement = (
            <RemoveTrackDialog
                track={track}
                list={ALL_MUSIC.name}
                onBackdropClick={closeDialog}
                onCancel={closeDialog}
                onConfirm={() => {
                    closeDialog();
                    run(commands.removeTrack(track.id).then(() => showToast("已从曲库移除", true)));
                }}
            />
        );
    }

    const mini =
        current && state ? (
            <MiniPlayer
                current={current}
                playing={playing}
                durationMs={state.durationMs}
                onOpen={openPlayer}
                onToggle={() => run(commands.toggle())}
                onNext={() => run(commands.next())}
            />
        ) : null;

    return (
        <main data-platform="android" className="android">
            {stack.map((entry, index) => {
                const position = {
                    level: index,
                    active: entry.key === topKey,
                    leaving: entry.leaving,
                };
                const page = entry.page;
                switch (page.kind) {
                    case "home":
                        return (
                            <HomePage
                                key={entry.key}
                                {...position}
                                lists={lists}
                                onOpen={id => push({ kind: "playlist", id })}
                                onCreate={() => setDialog({ type: "new-playlist" })}
                                mini={mini}
                            />
                        );
                    case "playlist": {
                        const info = describe(page.id);
                        const query = queries[entry.key] ?? "";
                        return (
                            <PlaylistPage
                                key={entry.key}
                                {...position}
                                id={page.id}
                                name={info.name}
                                builtIn={info.builtIn}
                                members={info.members}
                                currentId={currentId}
                                query={query}
                                onQueryChange={value => setQuery(entry.key, value)}
                                flash={flash}
                                locate={locate}
                                onLocateDone={clearLocate}
                                onPlay={(ids, start) =>
                                    playFrom(info.builtIn ? null : page.id, query.trim() ? ids : null, start)
                                }
                                onBack={pop}
                                onImport={() => push({ kind: "import" })}
                                onAdd={() => push({ kind: "add", id: page.id })}
                                onMenu={() => setSheet("playlist-menu")}
                                onRename={() => setDialog({ type: "rename-playlist", id: page.id })}
                                onDelete={() => setDialog({ type: "delete-playlist", id: page.id })}
                                menuOpen={sheet === "playlist-menu"}
                                onCloseMenu={() => setSheet(null)}
                                onRemove={track => {
                                    if (page.id === ALL_MUSIC.id) {
                                        setDialog({ type: "remove-track", track });
                                        return;
                                    }
                                    run(
                                        commands
                                            .removePlaylistTrack(page.id, track.id)
                                            .then(() => showToast(`已从「${info.name}」移除`, true)),
                                    );
                                }}
                                mini={mini}
                            />
                        );
                    }
                    case "add": {
                        const info = describe(page.id);
                        return (
                            <AddSongsPage
                                key={entry.key}
                                {...position}
                                playlistName={info.name}
                                tracks={library?.tracks ?? NO_TRACKS}
                                memberIds={new Set(info.members.map(track => track.id))}
                                query={queries[entry.key] ?? ""}
                                onQueryChange={value => setQuery(entry.key, value)}
                                onBack={pop}
                                onConfirm={ids =>
                                    run(
                                        commands.addPlaylistTracks(page.id, ids).then(added => {
                                            showToast(`已添加 ${added} 首到「${info.name}」`, true);
                                            pop();
                                        }),
                                    )
                                }
                            />
                        );
                    }
                    case "import":
                        return (
                            <ImportPage
                                key={entry.key}
                                {...position}
                                onBack={pop}
                                showToast={showToast}
                                onGoListen={added => {
                                    // 回到歌单页并滚到第一首导入的歌，不自动播放
                                    setFlash(new Set(added));
                                    setLocate(added[0] ?? null);
                                    pop();
                                }}
                            />
                        );
                    case "player":
                        return state ? (
                            <PlayerPage
                                key={entry.key}
                                {...position}
                                state={state}
                                sheet={sheet === "tune" || sheet === "queue" ? sheet : null}
                                onSheetChange={setSheet}
                                onQueueRemove={id =>
                                    run(
                                        commands
                                            .removeQueueTrack(id)
                                            .then(() => showToast("已从播放列表移除，歌单里仍保留", true)),
                                    )
                                }
                                onBack={pop}
                                showToast={showToast}
                            />
                        ) : null;
                }
            })}
            {dialogElement}
            <Toast
                message={toast?.message ?? null}
                variant="mobile"
                icon={toast?.done ? <Icon path={icons.check} /> : undefined}
                id={toast?.key}
                onDismiss={() => setToast(null)}
            />
        </main>
    );
}
