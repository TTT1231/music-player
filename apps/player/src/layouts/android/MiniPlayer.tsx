import { Cover } from "../../components/Cover";
import { LiveFill } from "../../components/Live";
import type { TrackView } from "../../ipc";
import { Icon, icons } from "./shared";

/** 首页和歌单页底部的迷你播放条；有歌在播时才出现。 */
export function MiniPlayer({
    current,
    playing,
    durationMs,
    onOpen,
    onToggle,
    onNext,
}: {
    current: TrackView;
    playing: boolean;
    durationMs: number | null;
    onOpen: () => void;
    onToggle: () => void;
    onNext: () => void;
}) {
    return (
        <div className="mini">
            <Cover id={current.id} title={current.title} coverUrl={current.coverUrl} size={42} />
            <button type="button" className="tx" onClick={onOpen}>
                <span className="t">{current.title}</span>
                <span className="a">{current.artist}</span>
            </button>
            <button type="button" className="mini-btn" aria-label={playing ? "暂停" : "播放"} onClick={onToggle}>
                <Icon path={playing ? icons.pause : icons.play} />
            </button>
            <button type="button" className="mini-btn" aria-label="下一首" onClick={onNext}>
                <Icon path={icons.next} />
            </button>
            <LiveFill className="p" durationMs={durationMs} />
        </div>
    );
}
