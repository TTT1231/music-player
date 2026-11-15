import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

const store = vi.hoisted(() => ({
    platform: null as "desktop" | "android" | null,
    start: vi.fn(),
    stop: vi.fn(),
}));

vi.mock("./store", () => ({
    playerStore: { start: store.start, stop: store.stop },
    usePlayerState: () => ({ platform: store.platform }),
}));

import App from "./App";

afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    store.platform = null;
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
