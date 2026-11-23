import { expect, test, vi } from "vitest";
import type { AppInfo, LibraryState, Notice, PlayerState, Position } from "./ipc";
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
const library = (values: Partial<LibraryState> = {}): LibraryState => ({
    tracks: [],
    playlists: [],
    ...values,
});

function harness(initial = player(), initialLibrary = library()) {
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
    let libraryChange: (value: LibraryState) => void = () => {};
    const unlisten = vi.fn();
    const deps: PlayerStoreDeps = {
        getState: vi.fn(async () => initial),
        appInfo: vi.fn(async (): Promise<AppInfo> => ({ platform: "android" })),
        getLibrary: vi.fn(async () => initialLibrary),
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
        listenLibrary: async callback => {
            libraryChange = callback;
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
        library: (value: LibraryState) => libraryChange(value),
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
    expect(h.store.getPosition()).toBe(1300);
    h.position(1500);
    h.advance(100);
    expect(h.store.getPosition()).toBe(1650);
    h.state(player({ playing: false, positionMs: 1600 }));
    h.advance(500);
    expect(h.store.getPosition()).toBe(1600);
    h.state(player({ currentId: "second", positionMs: 50, rate: 1 }));
    h.advance(100);
    expect(h.store.getPosition()).toBe(150);
    h.advance(5000);
    expect(h.store.getPosition()).toBe(3000);
    h.position(4000);
    expect(h.store.getPosition()).toBe(3000);
    h.store.stop();
    expect(h.frames.size).toBe(0);
    expect(h.unlisten).toHaveBeenCalledTimes(4);
});

test("progress frames and position events notify only position subscribers", async () => {
    const h = harness();
    await h.store.start();
    const onState = vi.fn();
    const onPosition = vi.fn();
    h.store.subscribe(onState);
    h.store.subscribePosition(onPosition);
    h.advance(16);
    h.advance(16);
    h.position(1100);
    h.advance(16);
    expect(onState).not.toHaveBeenCalled();
    expect(onPosition).toHaveBeenCalledTimes(4);
    expect(h.store.getSnapshot().state?.positionMs).toBe(1000);
    h.state(player({ positionMs: 2000 }));
    expect(onState).toHaveBeenCalledTimes(1);
    expect(h.store.getPosition()).toBe(2000);
});

test("the library is loaded with the player and follows its own event without touching progress", async () => {
    const initial = library({
        tracks: [{ id: "first", title: "一", artist: "a", hasLyrics: false, durationMs: null, coverUrl: "" }],
    });
    const h = harness(player(), initial);
    await h.store.start();
    expect(h.store.getSnapshot().library).toBe(initial);
    const onState = vi.fn();
    h.store.subscribe(onState);
    h.advance(200);
    const next = library({ playlists: [{ id: "p1", name: "夜跑", trackIds: ["first"] }] });
    h.library(next);
    expect(h.store.getSnapshot().library).toBe(next);
    // 曲库变化不重设进度插值的基准
    h.advance(100);
    expect(h.store.getPosition()).toBe(1300);
    expect(onState).toHaveBeenCalledTimes(1);
    expect(h.store.getSnapshot().state?.positionMs).toBe(1000);
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

test("returning to the foreground refreshes the projection from Rust", async () => {
    const h = harness();
    await h.store.start();
    const refreshed = library({ playlists: [{ id: "p1", name: "夜跑", trackIds: [] }] });
    vi.mocked(h.deps.getState).mockResolvedValueOnce(player({ playing: false, positionMs: 2400 }));
    vi.mocked(h.deps.getLibrary).mockResolvedValueOnce(refreshed);
    await h.store.refresh();
    expect(h.store.getSnapshot().state?.playing).toBe(false);
    expect(h.store.getPosition()).toBe(2400);
    expect(h.store.getSnapshot().library).toBe(refreshed);
    h.store.stop();
});

test("library event arriving during a refresh wins over the stale response", async () => {
    const h = harness();
    await h.store.start();
    let resolve!: (value: LibraryState) => void;
    vi.mocked(h.deps.getLibrary).mockReturnValueOnce(
        new Promise(r => {
            resolve = r;
        }),
    );
    const refreshing = h.store.refresh();
    await vi.waitFor(() => expect(resolve).toBeTypeOf("function"));
    const fresh = library({ playlists: [{ id: "p2", name: "白天", trackIds: [] }] });
    h.library(fresh);
    resolve(library());
    await refreshing;
    expect(h.store.getSnapshot().library).toBe(fresh);
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
    expect(h.store.getSnapshot().library).toEqual(library());
    h.notice({ kind: "unreadable", title: "gone" });
    expect(h.store.getSnapshot().notice?.title).toBe("gone");
    h.store.dismissNotice();
    expect(h.store.getSnapshot().notice).toBeNull();
    h.store.stop();
});
