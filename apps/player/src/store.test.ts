import { expect, test, vi } from "vitest";
import type { AppInfo, Notice, PlayerState, Position } from "./ipc";
import { createPlayerStore, type FrameClock, type PlayerStoreDeps } from "./store";

const player = (values: Partial<PlayerState> = {}): PlayerState => ({
    tracks: [],
    currentId: "first",
    playing: true,
    mode: "loop",
    volume: 0.8,
    rate: 1,
    pitch: 0,
    positionMs: 1000,
    durationMs: 3000,
    ...values,
});

function harness(initial = player()) {
    let time = 0;
    let id = 0;
    const frames = new Map<number, FrameRequestCallback>();
    const clock: FrameClock = {
        now: () => time,
        request: callback => {
            frames.set(++id, callback);
            return id;
        },
        cancel: handle => {
            frames.delete(handle);
        },
    };
    let state: (value: PlayerState) => void = () => {};
    let position: (value: Position) => void = () => {};
    let notice: (value: Notice) => void = () => {};
    const unlisten = vi.fn();
    const deps: PlayerStoreDeps = {
        getState: vi.fn(async () => initial),
        appInfo: vi.fn(async (): Promise<AppInfo> => ({ platform: "android" })),
        listenState: async callback => {
            state = callback;
            return unlisten;
        },
        listenPosition: async callback => {
            position = callback;
            return unlisten;
        },
        listenNotice: async callback => {
            notice = callback;
            return unlisten;
        },
    };
    const store = createPlayerStore(deps, clock);
    return {
        store,
        deps,
        unlisten,
        state: (value: PlayerState) => state(value),
        position: (ms: number) => position({ ms }),
        notice: (value: Notice) => notice(value),
        advance(ms: number) {
            time += ms;
            const callbacks = [...frames.values()];
            frames.clear();
            callbacks.forEach(cb => cb(time));
        },
        frames,
    };
}

test("projects platform, interpolates progress and resets at pause or track changes", async () => {
    const h = harness(player({ rate: 1.5 }));
    expect(h.store.getSnapshot().state).toBeNull();
    await h.store.start();
    expect(h.store.getSnapshot().platform).toBe("android");
    h.advance(200);
    expect(h.store.getSnapshot().state?.positionMs).toBe(1300);
    h.position(1500);
    h.advance(100);
    expect(h.store.getSnapshot().state?.positionMs).toBe(1650);
    h.state(player({ playing: false, positionMs: 1600 }));
    h.advance(500);
    expect(h.store.getSnapshot().state?.positionMs).toBe(1600);
    h.state(player({ currentId: "second", positionMs: 50, rate: 1 }));
    h.advance(100);
    expect(h.store.getSnapshot().state?.positionMs).toBe(150);
    h.advance(5000);
    expect(h.store.getSnapshot().state?.positionMs).toBe(3000);
    h.position(4000);
    expect(h.store.getSnapshot().state?.positionMs).toBe(3000);
    h.store.stop();
    expect(h.frames.size).toBe(0);
    expect(h.unlisten).toHaveBeenCalledTimes(3);
});

test("state event arriving during initial load wins over stale response", async () => {
    const h = harness();
    let resolve!: (value: PlayerState) => void;
    h.deps.getState = () =>
        new Promise(r => {
            resolve = r;
        });
    const started = h.store.start();
    await vi.waitFor(() => expect(resolve).toBeTypeOf("function"));
    h.state(player({ currentId: "new", positionMs: 0 }));
    resolve(player({ currentId: "old" }));
    await started;
    expect(h.store.getSnapshot().state?.currentId).toBe("new");
    h.store.stop();
});

test("stop during initialization cleans late listeners and allows a fresh start", async () => {
    const h = harness();
    let release!: (unlisten: () => void) => void;
    h.deps.listenState = () =>
        new Promise(r => {
            release = r;
        });
    const first = h.store.start();
    await vi.waitFor(() => expect(release).toBeTypeOf("function"));
    h.store.stop();
    const lateUnlisten = vi.fn();
    h.deps.listenState = async () => h.unlisten;
    const second = h.store.start();
    release(lateUnlisten);
    await Promise.all([first, second]);
    expect(lateUnlisten).toHaveBeenCalledOnce();
    expect(h.store.getSnapshot().platform).toBe("android");
    h.notice({ kind: "unreadable", title: "gone" });
    expect(h.store.getSnapshot().notice?.title).toBe("gone");
    h.store.dismissNotice();
    expect(h.store.getSnapshot().notice).toBeNull();
    h.store.stop();
});
