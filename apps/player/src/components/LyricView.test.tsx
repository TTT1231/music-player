import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { LyricView } from "./LyricView";

const lines = [
    { timeMs: 500, text: "第一句歌词" },
    { timeMs: 1500, text: "第二句歌词" },
    { timeMs: 2500, text: "第三句歌词" },
];

afterEach(() => {
    cleanup();
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
});

test("shows the instrumental message for an empty lyric", () => {
    render(<LyricView lines={[]} positionMs={0} onSeek={() => {}} />);
    expect(screen.getByText("纯音乐，请欣赏")).not.toBeNull();
    expect(screen.queryAllByRole("button")).toHaveLength(0);
});

test("highlights the current line and seeks to a clicked line", () => {
    const onSeek = vi.fn();
    const { rerender } = render(<LyricView lines={lines} positionMs={1000} onSeek={onSeek} />);
    expect(screen.getByText("第一句歌词").classList.contains("is-active")).toBe(true);
    rerender(<LyricView lines={lines} positionMs={2000} onSeek={onSeek} />);
    expect(screen.getByText("第一句歌词").classList.contains("past")).toBe(true);
    expect(screen.getByText("第二句歌词").classList.contains("is-active")).toBe(true);
    fireEvent.click(screen.getByText("第三句歌词"));
    expect(onSeek).toHaveBeenCalledWith(2500);
});

test("follows near the upper middle, suspends on manual scroll, then resumes", () => {
    vi.useFakeTimers();
    const scrollTo = vi.fn();
    vi.stubGlobal("matchMedia", () => ({ matches: false }));
    const { container, rerender } = render(<LyricView lines={lines} positionMs={1000} onSeek={() => {}} />);
    const view = container.querySelector(".lyrics") as HTMLElement;
    view.scrollTo = scrollTo;
    Object.defineProperty(view, "clientHeight", { value: 200 });
    view.getBoundingClientRect = () => ({ top: 10 }) as DOMRect;
    const second = screen.getByText("第二句歌词");
    second.getBoundingClientRect = () => ({ top: 170 }) as DOMRect;
    Object.defineProperty(second, "offsetHeight", { value: 20 });
    rerender(<LyricView lines={lines} positionMs={2000} onSeek={() => {}} />);
    expect(scrollTo).toHaveBeenLastCalledWith({ top: 86, behavior: "smooth" });

    fireEvent.wheel(view);
    scrollTo.mockClear();
    rerender(<LyricView lines={lines} positionMs={2600} onSeek={() => {}} />);
    expect(scrollTo).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(3000));
    expect(scrollTo).toHaveBeenCalledTimes(1);
});

test("re-centres the current line when the lyric area changes size", () => {
    let resized: (() => void) | null = null;
    vi.stubGlobal(
        "ResizeObserver",
        class {
            constructor(callback: () => void) {
                resized = callback;
            }
            observe() {}
            disconnect() {}
        },
    );
    const scrollTo = vi.fn();
    Element.prototype.scrollTo = scrollTo as unknown as typeof Element.prototype.scrollTo;
    render(<LyricView lines={lines} positionMs={1600} onSeek={() => {}} />);
    scrollTo.mockClear();
    act(() => resized?.());
    expect(scrollTo).toHaveBeenCalledOnce();
});
