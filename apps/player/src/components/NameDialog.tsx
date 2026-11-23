import { useLayoutEffect, useRef, useState } from "react";
import { isBackdrop, useModalDialog } from "./useModalDialog";

export interface NameDialogProps {
    title: string;
    /** 带出的原名称；新建时为空。 */
    value?: string;
    okLabel: string;
    onCancel: () => void;
    /** 提交去掉首尾空白后仍为空的名称不会发生；失败由调用方提示，对话框保持打开。 */
    onSubmit: (value: string) => Promise<void>;
}

/** 单行命名的对话框（新建、重命名歌单）。 */
export function NameDialog({ title, value = "", okLabel, onCancel, onSubmit }: NameDialogProps) {
    const dialog = useModalDialog();
    const input = useRef<HTMLInputElement>(null);
    const [text, setText] = useState(value);
    const [busy, setBusy] = useState(false);
    useLayoutEffect(() => {
        input.current?.focus();
        input.current?.select();
    }, []);
    const submit = async () => {
        if (!text.trim() || busy) return;
        setBusy(true);
        try {
            await onSubmit(text);
        } finally {
            setBusy(false);
        }
    };
    return (
        <dialog
            ref={dialog}
            className="app-dialog"
            aria-labelledby="name-dialog-title"
            onCancel={event => {
                event.preventDefault();
                onCancel();
            }}
            onClick={event => {
                if (isBackdrop(event)) onCancel();
            }}
        >
            <div className="app-dialog-inner">
                <form
                    onSubmit={event => {
                        event.preventDefault();
                        void submit();
                    }}
                >
                    <h2 id="name-dialog-title">{title}</h2>
                    <input
                        ref={input}
                        className="app-dialog-input"
                        aria-label={title}
                        placeholder="歌单名称"
                        maxLength={30}
                        autoComplete="off"
                        value={text}
                        onChange={event => setText(event.target.value)}
                    />
                    <div className="app-dialog-actions">
                        <button type="button" onClick={onCancel}>
                            取消
                        </button>
                        <button type="submit" className="is-primary" disabled={!text.trim() || busy}>
                            {okLabel}
                        </button>
                    </div>
                </form>
            </div>
        </dialog>
    );
}
