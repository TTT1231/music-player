import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useRef } from "react";

type DropHandlers = {
    onEnter: () => void;
    onLeave: () => void;
    onDrop: (paths: string[]) => void;
};

/** 在桌面页面存活期间只注册一次系统文件拖放监听。 */
export function useNativeImportDrop(handlers: DropHandlers): void {
    const current = useRef(handlers);
    useEffect(() => {
        current.current = handlers;
    }, [handlers]);
    useEffect(() => {
        let active = true;
        let remove: (() => void) | undefined;
        try {
            void getCurrentWebview()
                .onDragDropEvent(({ payload }) => {
                    if (!active) return;
                    if (payload.type === "enter") current.current.onEnter();
                    else if (payload.type === "leave") current.current.onLeave();
                    else if (payload.type === "drop") current.current.onDrop(payload.paths);
                })
                .then(unlisten => {
                    if (active) remove = unlisten;
                    else unlisten();
                })
                .catch(error => console.error("无法监听文件拖放", error));
        } catch (error) {
            console.error("无法监听文件拖放", error);
        }
        return () => {
            active = false;
            remove?.();
        };
    }, []);
}
