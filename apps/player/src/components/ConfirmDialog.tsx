import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { isBackdrop, useModalDialog } from "./useModalDialog";

export interface ConfirmDialogProps {
    title: string;
    description: ReactNode;
    okLabel: string;
    onCancel: () => void;
    /** 失败由调用方提示，对话框保持打开。 */
    onConfirm: () => Promise<void>;
}

/** 需要确认的危险操作（删除歌单）。确认按钮是红色的最后一个按钮。 */
export function ConfirmDialog({ title, description, okLabel, onCancel, onConfirm }: ConfirmDialogProps) {
    const dialog = useModalDialog();
    const cancel = useRef<HTMLButtonElement>(null);
    const [busy, setBusy] = useState(false);
    useLayoutEffect(() => {
        cancel.current?.focus();
    }, []);
    return (
        <dialog
            ref={dialog}
            className="app-dialog"
            role="alertdialog"
            aria-labelledby="confirm-dialog-title"
            aria-describedby="confirm-dialog-description"
            onCancel={event => {
                event.preventDefault();
                onCancel();
            }}
            onClick={event => {
                if (isBackdrop(event)) onCancel();
            }}
        >
            <div className="app-dialog-inner">
                <h2 id="confirm-dialog-title">{title}</h2>
                <p id="confirm-dialog-description">{description}</p>
                <div className="app-dialog-actions">
                    <button ref={cancel} type="button" onClick={onCancel}>
                        取消
                    </button>
                    <button
                        type="button"
                        disabled={busy}
                        onClick={() => {
                            setBusy(true);
                            void onConfirm().finally(() => setBusy(false));
                        }}
                    >
                        {okLabel}
                    </button>
                </div>
            </div>
        </dialog>
    );
}
