import { useSyncExternalStore } from "react";
import {
    commands,
    listenLibrary,
    listenNotice,
    listenPosition,
    listenState,
    type AppInfo,
    type LibraryState,
    type Notice,
    type Platform,
    type PlayerState,
    type Position,
} from "./ipc";

type Unlisten = () => void;
export type PlayerStoreDeps = {
    getState: () => Promise<PlayerState>;
    getLibrary: () => Promise<LibraryState>;
    appInfo: () => Promise<AppInfo>;
    listenState: (handler: (state: PlayerState) => void) => Promise<Unlisten>;
    listenPosition: (handler: (position: Position) => void) => Promise<Unlisten>;
    listenNotice: (handler: (notice: Notice) => void) => Promise<Unlisten>;
    listenLibrary: (handler: (library: LibraryState) => void) => Promise<Unlisten>;
};
export type FrameClock = {
    now: () => number;
    request: (callback: FrameRequestCallback) => number;
    cancel: (handle: number) => void;
};
export type PlayerSnapshot = {
    state: PlayerState | null;
    library: LibraryState | null;
    platform: Platform | null;
    notice: Notice | null;
    error: string | null;
};

const defaultDeps: PlayerStoreDeps = {
    getState: commands.getState,
    getLibrary: commands.getLibrary,
    appInfo: commands.appInfo,
    listenState,
    listenPosition,
    listenNotice,
    listenLibrary,
};
const defaultClock: FrameClock = {
    now: () => performance.now(),
    request: callback => requestAnimationFrame(callback),
    cancel: handle => cancelAnimationFrame(handle),
};

/** Rust 状态的只读投影，进度插值不改变播放状态。 */
export function createPlayerStore(deps: PlayerStoreDeps = defaultDeps, clock: FrameClock = defaultClock) {
    let snapshot: PlayerSnapshot = { state: null, library: null, platform: null, notice: null, error: null };
    const listeners = new Set<() => void>();
    // 进度按帧插值，只通知订阅进度的组件；整棵界面只在真实状态变化时重渲染
    const positionListeners = new Set<() => void>();
    let position = 0;
    let active = false;
    let generation = 0;
    let stateRevision = 0;
    let libraryRevision = 0;
    let unlisten: Unlisten[] = [];
    let starting: Promise<void> | null = null;
    let frame: number | null = null;
    let baseMs = 0;
    let baseTime = 0;

    const publish = (change: Partial<PlayerSnapshot>) => {
        snapshot = { ...snapshot, ...change };
        listeners.forEach(listener => listener());
    };
    const cancelFrame = () => {
        if (frame !== null) clock.cancel(frame);
        frame = null;
    };
    const publishPosition = (value: number) => {
        if (value === position) return;
        position = value;
        positionListeners.forEach(listener => listener());
    };
    const schedule = () => {
        if (!active || !snapshot.state?.playing || frame !== null) return;
        frame = clock.request(() => {
            frame = null;
            const state = snapshot.state;
            if (!active || !state?.playing) return;
            const estimate = Math.max(0, baseMs + Math.max(0, clock.now() - baseTime) * state.rate);
            const positionMs = state.durationMs === null ? estimate : Math.min(estimate, state.durationMs);
            publishPosition(positionMs);
            if (state.durationMs === null || positionMs < state.durationMs) schedule();
        });
    };
    const receiveState = (state: PlayerState) => {
        stateRevision++;
        cancelFrame();
        baseMs = state.positionMs;
        baseTime = clock.now();
        publish({ state });
        publishPosition(state.positionMs);
        schedule();
    };
    const receiveLibrary = (library: LibraryState) => {
        libraryRevision++;
        publish({ library });
    };
    const receivePosition = ({ ms }: Position) => {
        const state = snapshot.state;
        if (!state?.playing) return;
        stateRevision++;
        cancelFrame();
        baseMs = Math.max(0, state.durationMs === null ? ms : Math.min(ms, state.durationMs));
        baseTime = clock.now();
        publishPosition(baseMs);
        schedule();
    };
    const stop = () => {
        active = false;
        generation++;
        starting = null;
        cancelFrame();
        for (const dispose of unlisten.splice(0)) dispose();
    };
    const refresh = async () => {
        if (!active) return;
        const current = generation;
        const revision = stateRevision;
        const librarySeen = libraryRevision;
        try {
            const [state, library] = await Promise.all([deps.getState(), deps.getLibrary()]);
            if (!active || current !== generation) return;
            if (revision === stateRevision) receiveState(state);
            if (librarySeen === libraryRevision) receiveLibrary(library);
        } catch {
            // 保留上个快照，后续的播放操作仍会通过命令报告错误。
        }
    };
    const start = (): Promise<void> => {
        if (active) return starting ?? Promise.resolve();
        active = true;
        const current = ++generation;
        publish({ error: null });
        const initialize = async () => {
            try {
                const subscriptions = [
                    () =>
                        deps.listenState(value => {
                            if (active && generation === current) receiveState(value);
                        }),
                    () =>
                        deps.listenPosition(value => {
                            if (active && generation === current) receivePosition(value);
                        }),
                    () =>
                        deps.listenNotice(value => {
                            if (active && generation === current) publish({ notice: value });
                        }),
                    () =>
                        deps.listenLibrary(value => {
                            if (active && generation === current) receiveLibrary(value);
                        }),
                ];
                for (const subscribe of subscriptions) {
                    const dispose = await subscribe();
                    if (!active || generation !== current) {
                        dispose();
                        return;
                    }
                    unlisten.push(dispose);
                }
                const revision = stateRevision;
                const librarySeen = libraryRevision;
                const [state, app, library] = await Promise.all([deps.getState(), deps.appInfo(), deps.getLibrary()]);
                if (!active || generation !== current) return;
                if (stateRevision === revision) receiveState(state);
                if (libraryRevision === librarySeen) receiveLibrary(library);
                publish({ platform: app.platform });
            } catch (error) {
                if (active && generation === current) {
                    stop();
                    publish({ error: String(error) });
                }
            } finally {
                if (generation === current) starting = null;
            }
        };
        starting = initialize();
        return starting;
    };
    return {
        start,
        stop,
        refresh,
        getSnapshot: () => snapshot,
        subscribe(listener: () => void) {
            listeners.add(listener);
            return () => listeners.delete(listener);
        },
        /** 当前播放进度（毫秒），播放时按帧插值。 */
        getPosition: () => position,
        subscribePosition(listener: () => void) {
            positionListeners.add(listener);
            return () => positionListeners.delete(listener);
        },
        dismissNotice() {
            if (snapshot.notice) publish({ notice: null });
        },
    };
}

export const playerStore = createPlayerStore();
/** 播放器状态；其中 positionMs 只在状态事件时更新，实时进度用 usePlaybackPosition。 */
export function usePlayerState(): PlayerSnapshot {
    return useSyncExternalStore(playerStore.subscribe, playerStore.getSnapshot, playerStore.getSnapshot);
}
/** 实时播放进度（毫秒）。只在进度条、时间、歌词这类小组件里调用，避免整个布局按帧重渲染。 */
export function usePlaybackPosition(): number {
    return useSyncExternalStore(playerStore.subscribePosition, playerStore.getPosition, playerStore.getPosition);
}
export function usePlatform(): Platform | null {
    const platform = () => playerStore.getSnapshot().platform;
    return useSyncExternalStore(playerStore.subscribe, platform, platform);
}
/** 曲库和自建歌单；由 library 事件更新，不随播放进度重渲染。 */
export function useLibrary(): LibraryState | null {
    const library = () => playerStore.getSnapshot().library;
    return useSyncExternalStore(playerStore.subscribe, library, library);
}
