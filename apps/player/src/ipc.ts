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
};

export const listenState = (handler: (state: PlayerState) => void): Promise<UnlistenFn> =>
    listen<PlayerState>("state", event => handler(event.payload));
export const listenPosition = (handler: (position: Position) => void): Promise<UnlistenFn> =>
    listen<Position>("position", event => handler(event.payload));
export const listenNotice = (handler: (notice: Notice) => void): Promise<UnlistenFn> =>
    listen<Notice>("notice", event => handler(event.payload));
