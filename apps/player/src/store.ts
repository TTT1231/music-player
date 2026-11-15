import { useSyncExternalStore } from "react";
import {
    commands,
    listenNotice,
    listenPosition,
    listenState,
    type AppInfo,
    type Notice,
    type Platform,
    type PlayerState,
    type Position,
} from "./ipc";

type Unlisten = () => void;
export type PlayerStoreDeps = {
    getState: () => Promise<PlayerState>;
    appInfo: () => Promise<AppInfo>;
    listenState: (handler: (state: PlayerState) => void) => Promise<Unlisten>;
    listenPosition: (handler: (position: Position) => void) => Promise<Unlisten>;
    listenNotice: (handler: (notice: Notice) => void) => Promise<Unlisten>;
};
export type FrameClock = {
    now: () => number;
    request: (callback: FrameRequestCallback) => number;
    cancel: (handle: number) => void;
};
export type PlayerSnapshot = {
    state: PlayerState | null;
    platform: Platform | null;
    notice: Notice | null;
    error: string | null;
};

const defaultDeps: PlayerStoreDeps = {
    getState: commands.getState,
    appInfo: commands.appInfo,
    listenState,
    listenPosition,
    listenNotice,
};
const defaultClock: FrameClock = {
    now: () => performance.now(),
    request: callback => requestAnimationFrame(callback),
    cancel: handle => cancelAnimationFrame(handle),
};

/** Rust 状态的只读投影，进度插值不改变播放状态。 */
export function createPlayerStore(deps: PlayerStoreDeps = defaultDeps, clock: FrameClock = defaultClock) {
    let snapshot: PlayerSnapshot = { state: null, platform: null, notice: null, error: null };
    const listeners = new Set<() => void>();
    let active = false;
    let generation = 0;
    let stateRevision = 0;
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
    const schedule = () => {
        if (!active || !snapshot.state?.playing || frame !== null) return;
        frame = clock.request(() => {
            frame = null;
            const state = snapshot.state;
            if (!active || !state?.playing) return;
            const position = Math.max(0, baseMs + Math.max(0, clock.now() - baseTime) * state.rate);
            const positionMs = state.durationMs === null ? position : Math.min(position, state.durationMs);
            if (positionMs !== state.positionMs) publish({ state: { ...state, positionMs } });
            if (state.durationMs === null || positionMs < state.durationMs) schedule();
        });
    };
    const receiveState = (state: PlayerState) => {
        stateRevision++;
        cancelFrame();
        baseMs = state.positionMs;
        baseTime = clock.now();
        publish({ state });
        schedule();
    };
    const receivePosition = ({ ms }: Position) => {
        const state = snapshot.state;
        if (!state?.playing) return;
        stateRevision++;
        cancelFrame();
        baseMs = Math.max(0, state.durationMs === null ? ms : Math.min(ms, state.durationMs));
        baseTime = clock.now();
        publish({ state: { ...state, positionMs: baseMs } });
        schedule();
    };
    const stop = () => {
        active = false;
        generation++;
        starting = null;
        cancelFrame();
        for (const dispose of unlisten.splice(0)) dispose();
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
                const [state, app] = await Promise.all([deps.getState(), deps.appInfo()]);
                if (!active || generation !== current) return;
                if (stateRevision === revision) receiveState(state);
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
        getSnapshot: () => snapshot,
        subscribe(listener: () => void) {
            listeners.add(listener);
            return () => listeners.delete(listener);
        },
        dismissNotice() {
            if (snapshot.notice) publish({ notice: null });
        },
    };
}

export const playerStore = createPlayerStore();
export function usePlayerState(): PlayerSnapshot {
    return useSyncExternalStore(playerStore.subscribe, playerStore.getSnapshot, playerStore.getSnapshot);
}
