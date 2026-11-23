import { useEffect, useRef, useState, type ReactNode } from "react";
import { Cover } from "../../components/Cover";
import type { TrackView } from "../../ipc";
import {
    ALL_MUSIC,
    BackButton,
    Equalizer,
    Icon,
    Page,
    SearchBox,
    TrackText,
    icons,
    matches,
    type PageProps,
} from "./shared";

export type PlaylistPageProps = PageProps & {
    id: string;
    /** 歌单名称；曲库投影还没到时为空。 */
    name: string;
    /** 「全部音乐」即曲库：＋ 是导入，移除歌曲会删除曲库文件。 */
    builtIn: boolean;
    /** 歌单里的歌曲，按歌单顺序。 */
    members: TrackView[];
    currentId: string | null;
    query: string;
    onQueryChange: (value: string) => void;
    /** 刚导入、要短暂高亮的歌曲。 */
    flash: ReadonlySet<string>;
    /** 要滚动到视野内的歌曲；找到后调用 `onLocateDone`。 */
    locate: string | null;
    onLocateDone: () => void;
    /** 播放列表换成 `ids` 的顺序，从第二首参数开始。 */
    onPlay: (ids: string[], start: string) => void;
    onBack: () => void;
    onImport: () => void;
    onAdd: () => void;
    /** ⋯：打开歌单操作菜单（重命名、删除歌单）。 */
    onMenu: () => void;
    onRename: () => void;
    onDelete: () => void;
    menuOpen: boolean;
    onCloseMenu: () => void;
    /** 编辑模式下点行尾的 ×。 */
    onRemove: (track: TrackView) => void;
    mini: ReactNode;
};

/** 歌单页：封面、播放全部、加歌入口、搜索和歌曲列表。 */
export function PlaylistPage({
    level,
    active,
    leaving,
    id,
    name,
    builtIn,
    members,
    currentId,
    query,
    onQueryChange,
    flash,
    locate,
    onLocateDone,
    onPlay,
    onBack,
    onImport,
    onAdd,
    onMenu,
    onRename,
    onDelete,
    menuOpen,
    onCloseMenu,
    onRemove,
    mini,
}: PlaylistPageProps) {
    // 进入编辑模式只对这次打开有效：重新打开歌单时页面重建，自然回到浏览状态
    const [editing, setEditing] = useState(false);
    // 往下滚过封面后，顶栏才显示歌单名
    const [scrolled, setScrolled] = useState(false);
    const scroll = useRef<HTMLDivElement>(null);
    const searching = Boolean(query.trim());
    const tracks = searching ? members.filter(track => matches(track, query)) : members;
    const ids = tracks.map(track => track.id);
    const addLabel = builtIn ? "导入音乐和歌词" : "添加歌曲到歌单";

    // 导入后来到这里：把新导入的第一首歌滚到视野内
    useEffect(() => {
        if (!active || !locate) return;
        const row = scroll.current?.querySelector<HTMLElement>(`.m-row[data-id="${CSS.escape(locate)}"]`);
        if (!row) return;
        if (typeof row.scrollIntoView === "function") row.scrollIntoView({ block: "center" });
        onLocateDone();
    }, [active, locate, onLocateDone, members, query]);

    return (
        <Page kind="playlist" level={level} active={active} leaving={leaving} label="歌单">
            <div className="page-body">
                <div className={`pl-top${scrolled ? " is-scrolled" : ""}`}>
                    <BackButton onClick={onBack} />
                    <span className="pl-title">{name}</span>
                    {/* 这两个图标按钮只靠 aria-label 报出名字：安卓 WebView 一旦带上 aria-pressed、
                    aria-haspopup 或 aria-expanded，就不再把名字交给系统无障碍树 */}
                    {builtIn ? null : (
                        <button
                            type="button"
                            className="top-btn more"
                            aria-label="歌单更多操作"
                            title="歌单更多操作"
                            onClick={onMenu}
                        >
                            <Icon path={icons.more} />
                        </button>
                    )}
                    <button
                        type="button"
                        className="top-btn edit-toggle"
                        data-editing={editing}
                        aria-label={editing ? "完成编辑歌曲" : "编辑歌曲"}
                        title={editing ? "完成编辑歌曲" : "编辑歌曲"}
                        onClick={() => setEditing(value => !value)}
                    >
                        <Icon path={editing ? icons.check : icons.edit} />
                    </button>
                </div>
                <div
                    className="pl-scroll"
                    ref={scroll}
                    onScroll={event => setScrolled(event.currentTarget.scrollTop > 92)}
                >
                    <div className="pl-head">
                        <Cover id={id} title={name} size={60} colors={builtIn ? ALL_MUSIC.colors : undefined} />
                        <div className="pl-info">
                            <h1 className="pl-name">{name}</h1>
                            <p className="pl-count">{searching ? `找到 ${ids.length} 首` : `${members.length}首`}</p>
                        </div>
                    </div>
                    <div className="pl-actions">
                        <button
                            type="button"
                            className="pill play-all"
                            disabled={!ids.length}
                            onClick={() => ids.length && onPlay(ids, ids[0])}
                        >
                            <Icon path={icons.play} />
                            <span>播放全部</span>
                        </button>
                        <button
                            type="button"
                            className="add-btn"
                            aria-label={addLabel}
                            title={addLabel}
                            onClick={builtIn ? onImport : onAdd}
                        >
                            <Icon path={icons.plus} />
                        </button>
                    </div>
                    <SearchBox value={query} onChange={onQueryChange} />
                    <div className="m-list" role="list">
                        {tracks.length === 0 ? (
                            <p className="m-empty">
                                {searching
                                    ? "没有找到匹配的歌曲。"
                                    : builtIn
                                      ? "还没有音乐。点上面的 + 导入音乐和歌词。"
                                      : "歌单还是空的。点上面的 + 添加歌曲。"}
                            </p>
                        ) : (
                            tracks.map(track => {
                                const on = track.id === currentId;
                                return (
                                    <div
                                        key={track.id}
                                        role="listitem"
                                        data-id={track.id}
                                        className={`m-row${on ? " on" : ""}${flash.has(track.id) ? " is-flash" : ""}`}
                                    >
                                        <button type="button" className="m-play" onClick={() => onPlay(ids, track.id)}>
                                            <span className="cv">
                                                <Cover
                                                    id={track.id}
                                                    title={track.title}
                                                    coverUrl={track.coverUrl}
                                                    size={46}
                                                />
                                                {on && <Equalizer />}
                                            </span>
                                            <TrackText track={track} />
                                        </button>
                                        {editing && (
                                            <button
                                                type="button"
                                                className="m-del"
                                                aria-label={`从「${name}」移除：${track.title}`}
                                                title="移除歌曲"
                                                onClick={() => onRemove(track)}
                                            >
                                                <Icon path={icons.close} />
                                            </button>
                                        )}
                                    </div>
                                );
                            })
                        )}
                    </div>
                </div>
                {mini}
            </div>
            {menuOpen && (
                <>
                    <div className="scrim" onClick={onCloseMenu} />
                    <div className="sheet menu-sheet" role="menu" aria-label="歌单操作">
                        <div className="grab" />
                        <button
                            type="button"
                            className="menu-item"
                            role="menuitem"
                            onClick={() => {
                                onCloseMenu();
                                onRename();
                            }}
                        >
                            <Icon path={icons.edit} />
                            重命名
                        </button>
                        <button
                            type="button"
                            className="menu-item danger"
                            role="menuitem"
                            onClick={() => {
                                onCloseMenu();
                                onDelete();
                            }}
                        >
                            <Icon path={icons.trash} />
                            删除歌单
                        </button>
                    </div>
                </>
            )}
        </Page>
    );
}
