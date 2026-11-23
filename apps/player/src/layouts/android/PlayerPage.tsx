import { useEffect, useState } from "react";
import { LiveLyrics, LiveProgressBar, LiveTime } from "../../components/Live";
import { type LyricLine } from "../../components/LyricView";
import { MODES } from "../../components/ModeMenu";
import { TunePanel } from "../../components/TunePanel";
import { formatTime } from "../../components/formatTime";
import { commands, type PlayerState } from "../../ipc";
import { Snow } from "../Snow";
import { Cover } from "../../components/Cover";
import { Equalizer, Icon, Page, icons, reduceMotion, type PageProps } from "./shared";

export type PlayerSheet = "tune" | "queue";
/** 播放列表里的行先滑出这么久再真正移除。 */
const ROW_OUT_MS = 160;

export type PlayerPageProps = PageProps & {
    state: PlayerState;
    sheet: PlayerSheet | null;
    onSheetChange: (sheet: PlayerSheet | null) => void;
    /** 只移出播放列表：歌单和曲库不受影响。 */
    onQueueRemove: (id: string) => void;
    onBack: () => void;
    showToast: (message: string) => void;
};

/** 播放页：模糊封面 + 飘雪 + 歌词，以及升降调和播放列表两个底部弹层。 */
export function PlayerPage({
    level,
    active,
    leaving,
    state,
    sheet,
    onSheetChange,
    onQueueRemove,
    onBack,
    showToast,
}: PlayerPageProps) {
    const current = state.tracks.find(track => track.id === state.currentId);
    const [lines, setLines] = useState<LyricLine[] | null>(null);
    const [preview, setPreview] = useState<number | null>(null);
    const [removing, setRemoving] = useState<string | null>(null);
    const playing = state.playing;
    const mode = MODES.find(item => item.id === state.mode) ?? MODES[1];
    const rateChanged = Math.abs(state.rate - 1) > 0.01;

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
        const ids = MODES.map(item => item.id);
        const next = ids[(ids.indexOf(state.mode) + 1) % ids.length];
        const label = MODES.find(item => item.id === next)?.label ?? "";
        run(commands.setMode(next).then(() => showToast(label)));
    };

    /** 行先滑出再改数据 */
    const removeQueued = (id: string) => {
        if (removing) return;
        setRemoving(id);
        window.setTimeout(
            () => {
                setRemoving(null);
                onQueueRemove(id);
            },
            reduceMotion() ? 0 : ROW_OUT_MS,
        );
    };

    return (
        <Page kind="player" level={level} active={active} leaving={leaving} label="播放页">
            {current?.coverUrl && <div className="np-bg" style={{ backgroundImage: `url("${current.coverUrl}")` }} />}
            <div className="np-shade" aria-hidden="true" />
            <Snow className="np-snow" />
            <div className="page-body">
                <div className="np-top">
                    <button type="button" aria-label="收起" onClick={onBack}>
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
                            className={`np-rate${rateChanged || state.pitch !== 0 ? " changed" : ""}`}
                            aria-label="升降调和倍速"
                            title="升降调和倍速"
                            onClick={() => onSheetChange("tune")}
                        >
                            <Icon path={icons.dots} />
                            <sup>{rateChanged ? `${state.rate.toFixed(1)}X` : ""}</sup>
                        </button>
                    </div>
                    <LiveProgressBar
                        variant="mobile"
                        durationMs={state.durationMs}
                        onPreviewChange={setPreview}
                        onSeek={ms => run(commands.seek(ms))}
                    />
                    <div className="np-times">
                        <span>
                            <LiveTime preview={preview} />
                        </span>
                        <span>{formatTime(state.durationMs)}</span>
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
                            onClick={() => current && run(commands.toggle())}
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
                            onClick={() => onSheetChange("queue")}
                        >
                            <Icon path={icons.queue} />
                        </button>
                    </div>
                </div>
            </div>
            {sheet && (
                <>
                    <div className="scrim" data-testid="scrim" onClick={() => onSheetChange(null)} />
                    {sheet === "tune" ? (
                        <div className="sheet tune-sheet" role="dialog" aria-label="升降调和倍速">
                            <div className="grab" />
                            <TunePanel
                                rate={state.rate}
                                pitch={state.pitch}
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
                                <span> ({state.tracks.length})</span>
                            </div>
                            <div className="ls-list">
                                {state.tracks.length === 0 ? (
                                    <p className="m-empty">播放列表是空的。</p>
                                ) : (
                                    state.tracks.map(track => {
                                        const on = track.id === state.currentId;
                                        return (
                                            <div
                                                key={track.id}
                                                className={`ls-row${on ? " on" : ""}${removing === track.id ? " is-removing" : ""}`}
                                            >
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
                                                            <Equalizer
                                                                className={playing ? "ls-eq" : "ls-eq is-paused"}
                                                            />
                                                        )}
                                                    </span>
                                                    <span className="ls-tx">
                                                        <span className="t">{track.title}</span>
                                                        <span className="a">
                                                            {!track.hasLyrics && <span className="m-tag">纯音乐</span>}
                                                            <span>{track.artist}</span>
                                                        </span>
                                                    </span>
                                                </button>
                                                <button
                                                    type="button"
                                                    className="ls-del"
                                                    aria-label={`从播放列表移除：${track.title}`}
                                                    title="从播放列表移除"
                                                    onClick={() => removeQueued(track.id)}
                                                >
                                                    <Icon path={icons.close} />
                                                </button>
                                            </div>
                                        );
                                    })
                                )}
                            </div>
                        </div>
                    )}
                </>
            )}
        </Page>
    );
}
