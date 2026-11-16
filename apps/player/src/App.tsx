import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AndroidLayout } from "./layouts/AndroidLayout";
import { DesktopLayout } from "./layouts/DesktopLayout";
import { DesktopLyrics } from "./layouts/DesktopLyrics";
import { playerStore, usePlatform } from "./store";

/** 歌词窗口加载同一份前端：优先按 Tauri 窗口 label 识别，测试等无窗口环境退回查询参数。 */
function isLyricsWindow(): boolean {
    try {
        if (getCurrentWindow().label === "lyrics") return true;
    } catch {
        /* 非 Tauri 环境 */
    }
    return new URLSearchParams(window.location.search).get("window") === "lyrics";
}

export default function App() {
    const platform = usePlatform();
    const lyricsWindow = isLyricsWindow();

    useEffect(() => {
        void playerStore.start();
        return () => playerStore.stop();
    }, []);

    if (lyricsWindow) return <DesktopLyrics />;
    if (platform === "android") return <AndroidLayout />;
    if (platform === "desktop") return <DesktopLayout />;
    return <main />;
}
