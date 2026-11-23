import { useLayoutEffect, useRef } from "react";
import type { TrackView } from "../ipc";
import { isBackdrop, useModalDialog } from "./useModalDialog";

export function RemoveTrackDialog({
    track,
    list,
    onBackdropClick,
    onCancel,
    onConfirm,
}: {
    track: TrackView;
    /** 曲库入口的名称，例如安卓的「全部音乐」。 */
    list?: string;
    /** 点遮罩时调用；不传则点遮罩没有反应。 */
    onBackdropClick?: () => void;
    onCancel: () => void;
    onConfirm: () => void;
}) {
    const dialog = useModalDialog();
    const cancel = useRef<HTMLButtonElement>(null);
    useLayoutEffect(() => {
        cancel.current?.focus();
    }, []);

    return (
        <dialog
            ref={dialog}
            className="app-dialog"
            role="alertdialog"
            aria-labelledby="remove-track-title"
            aria-describedby="remove-track-description"
            onCancel={event => {
                event.preventDefault();
                onCancel();
            }}
            onClick={event => {
                if (isBackdrop(event)) onBackdropClick?.();
            }}
        >
            <div className="app-dialog-inner">
                <h2 id="remove-track-title">移除歌曲</h2>
                <p id="remove-track-description">
                    {list ? `确定从「${list}」移除《${track.title}》？` : `确定从曲库移除《${track.title}》？`}
                    曲库中的音乐文件{track.hasLyrics ? "和配对歌词" : ""}也会被删除。
                </p>
                <div className="app-dialog-actions">
                    <button ref={cancel} type="button" onClick={onCancel}>
                        取消
                    </button>
                    <button type="button" onClick={onConfirm}>
                        确认移除
                    </button>
                </div>
            </div>
        </dialog>
    );
}
