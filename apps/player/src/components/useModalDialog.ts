import { useLayoutEffect, useRef, type MouseEvent } from "react";
import "./Dialog.css";

/**
 * 打开一个原生 `<dialog>`：置顶显示，关闭后把焦点还给触发它的元素。
 * Esc 由各对话框的 `onCancel` 处理；点遮罩见 [`isBackdrop`]。
 */
export function useModalDialog() {
    const dialog = useRef<HTMLDialogElement>(null);
    useLayoutEffect(() => {
        const element = dialog.current!;
        const trigger = document.activeElement;
        element.showModal();
        return () => {
            element.close();
            if (trigger instanceof HTMLElement) trigger.focus();
        };
    }, []);
    return dialog;
}

/** 是否点在了遮罩上。内容放在 `.app-dialog-inner` 里，所以点到 `<dialog>` 自身只可能是点在遮罩上。 */
export function isBackdrop(event: MouseEvent<HTMLDialogElement>) {
    return event.target === event.currentTarget;
}
