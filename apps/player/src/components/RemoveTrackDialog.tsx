import { useLayoutEffect, useRef } from "react";
import type { TrackView } from "../ipc";
import "./RemoveTrackDialog.css";

export function RemoveTrackDialog({
    track,
    onCancel,
    onConfirm,
}: {
    track: TrackView;
    onCancel: () => void;
    onConfirm: () => void;
}) {
    const dialog = useRef<HTMLDialogElement>(null);
    const cancel = useRef<HTMLButtonElement>(null);
    useLayoutEffect(() => {
        const element = dialog.current!;
        const trigger = document.activeElement;
        element.showModal();
        cancel.current?.focus();
        return () => {
            element.close();
            if (trigger instanceof HTMLElement) trigger.focus();
        };
    }, []);

    return (
        <dialog
            ref={dialog}
            className="remove-track-dialog"
            role="alertdialog"
            aria-labelledby="remove-track-title"
            aria-describedby="remove-track-description"
            onCancel={event => {
                event.preventDefault();
                onCancel();
            }}
        >
            <h2 id="remove-track-title">移除歌曲</h2>
            <p id="remove-track-description">
                确定从播放列表移除《{track.title}》？曲库中的音乐文件{track.hasLyrics ? "和配对歌词" : ""}也会被删除。
            </p>
            <div className="remove-track-actions">
                <button ref={cancel} type="button" onClick={onCancel}>
                    取消
                </button>
                <button type="button" onClick={onConfirm}>
                    确认移除
                </button>
            </div>
        </dialog>
    );
}
