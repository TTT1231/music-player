import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

const store = vi.hoisted(() => ({
    platform: null as "desktop" | "android" | null,
    start: vi.fn(),
    stop: vi.fn(),
    windowLabel: "main",
}));

vi.mock("./store", () => ({
    playerStore: { start: store.start, stop: store.stop },
    usePlatform: () => store.platform,
    usePlayerState: () => ({ state: null, notice: null, error: null, platform: store.platform }),
    usePlaybackPosition: () => 0,
}));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ label: store.windowLabel }) }));
vi.mock("./layouts/DesktopLayout", () => ({ DesktopLayout: () => <main data-platform="desktop" /> }));
vi.mock("./layouts/DesktopLyrics", () => ({ DesktopLyrics: () => <main data-platform="desktop-lyrics" /> }));

import App from "./App";

afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    store.platform = null;
    store.windowLabel = "main";
});

test("starts and stops the player projection with the application", () => {
    const view = render(<App />);
    expect(store.start).toHaveBeenCalledOnce();
    view.unmount();
    expect(store.stop).toHaveBeenCalledOnce();
});

test("chooses the layout using the reported platform", () => {
    store.platform = "android";
    const view = render(<App />);
    expect(view.container.querySelector("main")?.dataset.platform).toBe("android");
    store.platform = "desktop";
    view.rerender(<App />);
    expect(view.container.querySelector("main")?.dataset.platform).toBe("desktop");
});

test("renders the desktop lyrics window when the page requests it", () => {
    window.history.replaceState(null, "", "/?window=lyrics");
    store.platform = "desktop";
    const view = render(<App />);
    expect(view.container.querySelector("main")?.dataset.platform).toBe("desktop-lyrics");
    window.history.replaceState(null, "", "/");
});

test("renders the desktop lyrics window when the Tauri window label is lyrics", () => {
    store.windowLabel = "lyrics";
    store.platform = "desktop";
    const view = render(<App />);
    expect(view.container.querySelector("main")?.dataset.platform).toBe("desktop-lyrics");
});
