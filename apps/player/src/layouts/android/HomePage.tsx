import type { ReactNode } from "react";
import { Cover } from "../../components/Cover";
import { ALL_MUSIC, Icon, Page, icons, type PageProps } from "./shared";

export type HomePageProps = PageProps & {
    /** 歌单列表：内置的「全部音乐」在前，其后是自建歌单。 */
    lists: ReadonlyArray<{ id: string; name: string; count: number; builtIn?: boolean }>;
    onOpen: (id: string) => void;
    onCreate: () => void;
    mini: ReactNode;
};

/** 首页：歌单列表。 */
export function HomePage({ level, active, leaving, lists, onOpen, onCreate, mini }: HomePageProps) {
    return (
        <Page kind="home" level={level} active={active} leaving={leaving} label="歌单">
            <div className="page-body">
                <div className="home-top">
                    <h1>歌单</h1>
                    <button type="button" className="top-btn" aria-label="新建歌单" title="新建歌单" onClick={onCreate}>
                        <Icon path={icons.plus} />
                    </button>
                </div>
                <div className="home-list" role="list">
                    {lists.map(list => (
                        <div key={list.id} className="pl-row" role="listitem">
                            <button type="button" className="pl-open" onClick={() => onOpen(list.id)}>
                                <Cover
                                    id={list.id}
                                    title={list.name}
                                    size={56}
                                    colors={list.builtIn ? ALL_MUSIC.colors : undefined}
                                />
                                <span className="pl-tx">
                                    <span className="t">{list.name}</span>
                                    <span className="a">{list.count}首</span>
                                </span>
                            </button>
                        </div>
                    ))}
                </div>
                {mini}
            </div>
        </Page>
    );
}
