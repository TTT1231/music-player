import { onBackButtonPress } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Platform = "desktop" | "android";
export type PlayMode = "order" | "loop" | "shuffle" | "repeat_one" | "single";
export type AppInfo = { platform: Platform };
export type TrackView = {
    id: string;
    title: string;
    artist: string;
    hasLyrics: boolean;
    durationMs: number | null;
    coverUrl: string;
};
export type PlayerState = {
    tracks: TrackView[];
    currentId: string | null;
    playing: boolean;
    mode: PlayMode;
    volume: number;
    rate: number;
    pitch: number;
    positionMs: number;
    durationMs: number | null;
};
export type ImportResult = {
    added: string[];
    withLyrics: number;
    lyricsAttached: number;
    orphanLyrics: number;
};
export type Lyrics = { lines: { timeMs: number; text: string }[] };
export type Position = { ms: number };
export type NotifyState = {
    playing: boolean;
    title: string | null;
    artist: string | null;
    durationMs: number | null;
    positionMs: number;
    rate: number;
};
export type Notice = { kind: "unreadable"; title: string };

export const commands = {
    appInfo: () => invoke<AppInfo>("app_info"),
    getState: () => invoke<PlayerState>("get_state"),
    importPaths: (paths: string[]) => invoke<ImportResult>("import_paths", { paths }),
    pickAndImport: (kind: "files" | "folder") => invoke<ImportResult | null>("pick_and_import", { kind }),
    playTrack: (id: string) => invoke<void>("play_track", { id }),
    removeTrack: (id: string) => invoke<void>("remove_track", { id }),
    play: () => invoke<void>("play"),
    pause: () => invoke<void>("pause"),
    toggle: () => invoke<void>("toggle"),
    next: () => invoke<void>("next"),
    prev: () => invoke<void>("prev"),
    seek: (ms: number) => invoke<void>("seek", { ms }),
    setMode: (mode: PlayMode) => invoke<void>("set_mode", { mode }),
    setVolume: (volume: number) => invoke<void>("set_volume", { volume }),
    setRate: (rate: number) => invoke<number>("set_rate", { rate }),
    setPitch: (semitones: number) => invoke<number>("set_pitch", { semitones }),
    moveTrack: (from: number, to: number) => invoke<void>("move_track", { from, to }),
    getLyrics: (id: string) => invoke<Lyrics>("get_lyrics", { id }),
    toggleDesktopLyrics: () => invoke<boolean>("toggle_desktop_lyrics"),
    // 以下命令只在安卓有效（packages/tauri-plugin-playback），其他平台为空操作
    /** 同步通知栏和锁屏卡片；进度由系统按位置、更新时间和倍速推算，只需在状态变化时调用 */
    notifyState: (state: NotifyState) => invoke<void>("plugin:playback|notify_state", state),
    startPlaybackEvents: () => invoke<void>("plugin:playback|start_events"),
    /** 把应用退到后台（同按主页键）：音乐继续播放。系统返回键在首页时用它代替关闭应用 */
    moveToBackground: () => invoke<void>("plugin:playback|move_to_background"),
};

/**
 * 接管安卓系统返回键：注册期间不再执行默认的“关闭应用”（关闭会结束整个进程，音乐随之停止）。
 * 返回取消注册的函数。
 */
export const onBackButton = async (handler: () => void): Promise<() => void> => {
    const listener = await onBackButtonPress(() => handler());
    return () => void listener.unregister();
};

/** 安卓原生事件（音频焦点丢失、耳机拔出、通知栏按钮）。 */
export type PlaybackEvent =
    | { kind: "pause"; reason: "focus" | "noisy" | "notification" }
    | { kind: "command"; action: "toggle" | "next" | "prev" };

export const listenPlaybackEvents = (handler: (event: PlaybackEvent) => void): Promise<UnlistenFn> =>
    listen<PlaybackEvent>("playback://event", event => handler(event.payload));

export const listenState = (handler: (state: PlayerState) => void): Promise<UnlistenFn> =>
    listen<PlayerState>("state", event => handler(event.payload));
export const listenPosition = (handler: (position: Position) => void): Promise<UnlistenFn> =>
    listen<Position>("position", event => handler(event.payload));
export const listenNotice = (handler: (notice: Notice) => void): Promise<UnlistenFn> =>
    listen<Notice>("notice", event => handler(event.payload));
