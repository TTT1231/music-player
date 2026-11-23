import { useEffect, useRef, useState } from "react";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { Cover } from "../components/Cover";
import { formatTime } from "../components/formatTime";
import type { SavedPlaylist, TrackView } from "../ipc";
import { PlaylistContextMenu, type PlaylistMenuPosition } from "./PlaylistContextMenu";
import "./PlaylistView.css";

type Props = {
    library: TrackView[];
    playlists: SavedPlaylist[];
    currentId: string | null;
    playing: boolean;
    onCreate: (name: string) => Promise<string>;
    onAdd: (id: string, tracks: string[]) => Promise<number>;
    onDelete: (id: string) => Promise<void>;
    onRemove: (id: string, track: string) => void;
    onRemoveLibrary?: (track: string) => void;
    onPlay: (id: string | null, track: string | null) => void;
    onImport: () => void;
};

export function PlaylistView({
    library,
    playlists,
    currentId,
    playing,
    onCreate,
    onAdd,
    onDelete,
    onRemove,
    onRemoveLibrary,
    onPlay,
    onImport,
}: Props) {
    const [selected, setSelected] = useState<string | null>(null);
    const [creating, setCreating] = useState(false);
    const [name, setName] = useState("");
    const [error, setError] = useState("");
    const [query, setQuery] = useState("");
    const [picking, setPicking] = useState(false);
    const [pickerQuery, setPickerQuery] = useState("");
    const [picked, setPicked] = useState<string[]>([]);
    const [menu, setMenu] = useState<PlaylistMenuPosition | null>(null);
    const [deletion, setDeletion] = useState<{ id: string; name: string; error?: string } | null>(null);
    const navigation = useRef<HTMLElement>(null);
    const selectAll = useRef<HTMLInputElement>(null);
    const active = playlists.find(list => list.id === selected);
    const activeId = active?.id ?? null;
    const ids = active?.trackIds ?? [];
    const byId = new Map(library.map(track => [track.id, track]));
    const tracks = active ? ids.flatMap(id => (byId.get(id) ? [byId.get(id)!] : [])) : library;
    const matches = (track: TrackView, value: string) =>
        `${track.title} ${track.artist}`.toLocaleLowerCase().includes(value.trim().toLocaleLowerCase());
    const visible = tracks.filter(track => matches(track, query));
    const options = library.filter(track => matches(track, pickerQuery));
    const selectable = options.filter(track => !ids.includes(track.id));
    const selectedVisible = selectable.filter(track => picked.includes(track.id)).length;
    const duplicate = ["全部音乐", ...playlists.map(list => list.name)].some(
        value => value.toLocaleLowerCase() === name.trim().toLocaleLowerCase(),
    );
    useEffect(() => {
        if (selectAll.current)
            selectAll.current.indeterminate = selectedVisible > 0 && selectedVisible < selectable.length;
    }, [selectedVisible, selectable.length]);
    const choose = (id: string | null) => {
        setSelected(id);
        setQuery("");
        setPicking(false);
        setCreating(false);
    };
    const closePicker = () => {
        setPicking(false);
        setPicked([]);
        setPickerQuery("");
    };
    return (
        <div
            className="playlist-screen"
            onKeyDown={event => {
                if (event.key === "Escape" && (picking || creating)) {
                    event.stopPropagation();
                    closePicker();
                    setCreating(false);
                }
            }}
        >
            <aside className="playlist-sidebar" aria-label="我的歌单">
                <div className="playlist-sidebar-title">
                    <h2>我的歌单</h2>
                    <button
                        type="button"
                        aria-label="新建歌单"
                        title="新建歌单"
                        onClick={() => {
                            setCreating(!creating);
                            setName("");
                            setError("");
                        }}
                    >
                        ＋
                    </button>
                </div>
                {creating && (
                    <form
                        className="playlist-create"
                        onSubmit={event => {
                            event.preventDefault();
                            if (!name.trim() || duplicate) return;
                            void onCreate(name.trim())
                                .then(id => {
                                    setSelected(id);
                                    setCreating(false);
                                    setQuery("");
                                })
                                .catch(reason => setError(String(reason)));
                        }}
                    >
                        <label htmlFor="playlist-name">歌单名称</label>
                        <input
                            id="playlist-name"
                            type="text"
                            value={name}
                            maxLength={40}
                            autoFocus
                            placeholder="例如：我的歌单"
                            onChange={event => {
                                setName(event.target.value);
                                setError("");
                            }}
                            aria-invalid={duplicate}
                        />
                        {duplicate && <p role="status">这个名称已被使用。</p>}
                        {error && <p role="alert">{error}</p>}
                        <div>
                            <button type="button" onClick={() => setCreating(false)}>
                                取消
                            </button>
                            <button className="playlist-primary" type="submit" disabled={!name.trim() || duplicate}>
                                创建
                            </button>
                        </div>
                    </form>
                )}
                <nav ref={navigation} aria-label="已保存的歌单">
                    <button
                        type="button"
                        aria-current={activeId === null ? "page" : undefined}
                        onClick={() => choose(null)}
                        onContextMenu={event => {
                            event.preventDefault();
                            setMenu(null);
                        }}
                    >
                        <span>☷ &nbsp;全部音乐</span>
                        <span>{library.length}</span>
                    </button>
                    {playlists.map(list => (
                        <button
                            type="button"
                            key={list.id}
                            aria-current={selected === list.id ? "page" : undefined}
                            aria-haspopup="menu"
                            aria-expanded={menu?.playlist.id === list.id}
                            aria-keyshortcuts="Shift+F10"
                            onClick={() => choose(list.id)}
                            onContextMenu={event => {
                                event.preventDefault();
                                setMenu({
                                    playlist: list,
                                    trigger: event.currentTarget,
                                    x: event.clientX,
                                    y: event.clientY,
                                });
                            }}
                            onKeyDown={event => {
                                if (event.key !== "ContextMenu" && !(event.shiftKey && event.key === "F10")) return;
                                event.preventDefault();
                                const trigger = event.currentTarget;
                                const { left, bottom } = trigger.getBoundingClientRect();
                                setMenu({ playlist: list, trigger, x: left, y: bottom });
                            }}
                        >
                            <span>♫ &nbsp;{list.name}</span>
                            <span>{list.trackIds.length}</span>
                        </button>
                    ))}
                </nav>
            </aside>
            <section className="playlist-library" aria-label="歌单详情">
                <header className="playlist-header">
                    <div>
                        <h1>{active?.name ?? "全部音乐"}</h1>
                        <p>{tracks.length} 首歌曲</p>
                    </div>
                    <div className="playlist-actions">
                        <button
                            type="button"
                            className="playlist-primary"
                            aria-label="播放全部"
                            disabled={!tracks.length}
                            onClick={() => onPlay(activeId, null)}
                        >
                            ▶ &nbsp;播放全部
                        </button>
                        <button type="button" onClick={() => (active ? setPicking(!picking) : onImport())}>
                            {active ? "添加歌曲" : "导入音乐"}
                        </button>
                    </div>
                    {tracks.length > 0 && (
                        <input
                            className="playlist-search"
                            type="search"
                            placeholder="搜索歌单"
                            aria-label="搜索当前歌单"
                            value={query}
                            onChange={event => setQuery(event.target.value)}
                        />
                    )}
                    {picking && active && (
                        <section className="playlist-picker" role="dialog" aria-label={`添加到「${active.name}」`}>
                            <header>
                                <h2>添加到「{active.name}」</h2>
                                <button type="button" aria-label="关闭选歌" onClick={closePicker}>
                                    ×
                                </button>
                            </header>
                            <input
                                autoFocus
                                type="search"
                                placeholder="搜索曲库"
                                aria-label="搜索曲库中的歌名或歌手"
                                value={pickerQuery}
                                onChange={event => setPickerQuery(event.target.value)}
                            />
                            <label className="playlist-select-all">
                                <input
                                    ref={selectAll}
                                    type="checkbox"
                                    disabled={!selectable.length}
                                    checked={!!selectable.length && selectedVisible === selectable.length}
                                    onChange={event =>
                                        setPicked(previous =>
                                            event.target.checked
                                                ? Array.from(
                                                      new Set([...previous, ...selectable.map(track => track.id)]),
                                                  )
                                                : previous.filter(id => !selectable.some(track => track.id === id)),
                                        )
                                    }
                                />
                                全选
                            </label>
                            <div className="playlist-picker-list">
                                {options.map(track => (
                                    <label key={track.id} className="playlist-picker-row">
                                        <input
                                            type="checkbox"
                                            aria-label={`选择 ${track.title}`}
                                            checked={ids.includes(track.id) || picked.includes(track.id)}
                                            disabled={ids.includes(track.id)}
                                            onChange={event =>
                                                setPicked(previous =>
                                                    event.target.checked
                                                        ? [...previous, track.id]
                                                        : previous.filter(id => id !== track.id),
                                                )
                                            }
                                        />
                                        <Cover id={track.id} title={track.title} coverUrl={track.coverUrl} size={32} />
                                        <span>
                                            <strong>{track.title}</strong>
                                            <small>{track.artist}</small>
                                        </span>
                                        <small>
                                            {ids.includes(track.id) ? "已添加" : formatTime(track.durationMs)}
                                        </small>
                                    </label>
                                ))}
                                {!options.length && <p>没有找到匹配的歌曲。</p>}
                            </div>
                            <footer>
                                <span>已选 {picked.length} 首</span>
                                <button type="button" onClick={closePicker}>
                                    取消
                                </button>
                                <button
                                    type="button"
                                    className="playlist-primary"
                                    disabled={!picked.length}
                                    onClick={() => {
                                        void onAdd(active.id, picked)
                                            .then(closePicker)
                                            .catch(reason => setError(String(reason)));
                                    }}
                                >
                                    添加
                                </button>
                            </footer>
                            {error && <p role="alert">{error}</p>}
                        </section>
                    )}
                </header>
                <div className="playlist-table-wrap">
                    {visible.length ? (
                        <table aria-label="当前歌单的歌曲">
                            <thead>
                                <tr>
                                    <th>#</th>
                                    <th>歌名</th>
                                    <th>歌手</th>
                                    <th aria-label="歌曲操作" />
                                    <th>时长</th>
                                </tr>
                            </thead>
                            <tbody>
                                {visible.map(track => (
                                    <tr key={track.id} className={currentId === track.id ? "is-current" : ""}>
                                        <td>
                                            {currentId === track.id ? (playing ? "♫" : "▶") : tracks.indexOf(track) + 1}
                                        </td>
                                        <td>
                                            <button
                                                className="playlist-track"
                                                type="button"
                                                aria-label={`播放 ${track.title}`}
                                                onClick={() => onPlay(activeId, track.id)}
                                            >
                                                <Cover
                                                    id={track.id}
                                                    title={track.title}
                                                    coverUrl={track.coverUrl}
                                                    size={32}
                                                />
                                                <span>{track.title}</span>
                                            </button>
                                        </td>
                                        <td>{track.artist}</td>
                                        <td>
                                            <button
                                                type="button"
                                                className="playlist-remove"
                                                aria-label={
                                                    active
                                                        ? `从当前歌单移出 ${track.title}`
                                                        : `从曲库移除 ${track.title}`
                                                }
                                                onClick={() =>
                                                    active ? onRemove(active.id, track.id) : onRemoveLibrary?.(track.id)
                                                }
                                            >
                                                ×
                                            </button>
                                        </td>
                                        <td>{formatTime(track.durationMs)}</td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    ) : (
                        <p className="playlist-empty">{query ? "没有找到匹配的歌曲。" : "这个歌单还没有歌曲。"}</p>
                    )}
                </div>
            </section>
            {menu && (
                <PlaylistContextMenu
                    menu={menu}
                    onDismiss={() => setMenu(null)}
                    onDelete={() => {
                        menu.trigger.focus({ preventScroll: true });
                        setMenu(null);
                        const { id, name } = menu.playlist;
                        setDeletion({ id, name });
                    }}
                />
            )}
            {deletion && (
                <ConfirmDialog
                    title="删除歌单"
                    description={
                        <>
                            {`确定删除「${deletion.name}」？歌单里的歌曲仍保留在「全部音乐」。`}
                            {deletion.error && (
                                <span role="alert" className="playlist-delete-error">
                                    {deletion.error}
                                </span>
                            )}
                        </>
                    }
                    okLabel="删除"
                    onCancel={() => setDeletion(null)}
                    onConfirm={async () => {
                        try {
                            await onDelete(deletion.id);
                            setSelected(current => (current === deletion.id ? null : current));
                            if (selected === deletion.id) {
                                setQuery("");
                                closePicker();
                            }
                            setDeletion(current => (current === deletion ? null : current));
                            requestAnimationFrame(() => {
                                if (document.activeElement === document.body)
                                    navigation.current
                                        ?.querySelector<HTMLButtonElement>('[aria-current="page"]')
                                        ?.focus();
                            });
                        } catch (reason) {
                            setDeletion(current =>
                                current === deletion ? { ...deletion, error: String(reason) } : current,
                            );
                        }
                    }}
                />
            )}
        </div>
    );
}
