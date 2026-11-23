import { useRef, useState } from "react";
import { Cover } from "../../components/Cover";
import type { TrackView } from "../../ipc";
import { BackButton, Icon, Page, SearchBox, TrackText, icons, matches, type PageProps } from "./shared";

export type AddSongsPageProps = PageProps & {
    playlistName: string;
    /** 「全部音乐」里的全部歌曲，按导入顺序。 */
    tracks: TrackView[];
    /** 已经在歌单里的歌曲，不能再选。 */
    memberIds: ReadonlySet<string>;
    query: string;
    onQueryChange: (value: string) => void;
    /** 按点选的顺序提交。 */
    onConfirm: (ids: string[]) => void;
    onBack: () => void;
};

/** 选歌页：从「全部音乐」里挑歌加进自建歌单。 */
export function AddSongsPage({
    level,
    active,
    leaving,
    playlistName,
    tracks,
    memberIds,
    query,
    onQueryChange,
    onConfirm,
    onBack,
}: AddSongsPageProps) {
    // 每次打开都从空的选择开始
    const [picked, setPicked] = useState<string[]>([]);
    const list = useRef<HTMLDivElement>(null);
    const view = tracks.filter(track => matches(track, query));
    // 全选针对所有还没加入的歌，不受搜索过滤影响
    const eligible = tracks.filter(track => !memberIds.has(track.id)).map(track => track.id);
    const allOn = eligible.length > 0 && eligible.every(id => picked.includes(id));

    const toggle = (id: string) =>
        setPicked(previous => (previous.includes(id) ? previous.filter(item => item !== id) : [...previous, id]));
    const toggleAll = () => setPicked(allOn ? [] : eligible);

    return (
        <Page kind="add" level={level} active={active} leaving={leaving} label="添加歌曲">
            <div className="page-body">
                <div className="add-top">
                    <BackButton onClick={onBack} />
                    <h1 className="add-title">添加到「{playlistName}」</h1>
                    {eligible.length ? (
                        <button type="button" className="add-all" aria-pressed={allOn} onClick={toggleAll}>
                            <span className="pk-box" aria-hidden="true">
                                <Icon path={icons.check} />
                            </span>
                            全选
                        </button>
                    ) : (
                        <span className="add-spacer" />
                    )}
                </div>
                <SearchBox value={query} onChange={onQueryChange} />
                <div className="m-list add-list" role="list" ref={list}>
                    {view.length === 0 ? (
                        <p className="m-empty">
                            {query.trim() ? "没有找到匹配的歌曲。" : "「全部音乐」里还没有歌曲。"}
                        </p>
                    ) : (
                        view.map(track => {
                            const inPlaylist = memberIds.has(track.id);
                            const on = picked.includes(track.id);
                            return (
                                <div key={track.id} role="listitem">
                                    <button
                                        type="button"
                                        className={`pk-row${on ? " is-on" : ""}${inPlaylist ? " is-in" : ""}`}
                                        aria-pressed={on}
                                        aria-label={inPlaylist ? `已在歌单中：${track.title}` : undefined}
                                        disabled={inPlaylist}
                                        onClick={() => toggle(track.id)}
                                    >
                                        <span className="pk-box" aria-hidden="true">
                                            <Icon path={icons.check} />
                                        </span>
                                        <span className="cv">
                                            <Cover
                                                id={track.id}
                                                title={track.title}
                                                coverUrl={track.coverUrl}
                                                size={46}
                                            />
                                        </span>
                                        <TrackText track={track} />
                                    </button>
                                </div>
                            );
                        })
                    )}
                </div>
                <div className="add-bar">
                    <button type="button" className="pill" disabled={!picked.length} onClick={() => onConfirm(picked)}>
                        {picked.length ? `添加 (${picked.length})` : "添加"}
                    </button>
                </div>
            </div>
        </Page>
    );
}
