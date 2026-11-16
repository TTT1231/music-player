import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { QueuePanel, type QueuePanelProps } from "./QueuePanel";

const tracks = [
    { id: "a", title: "春天", artist: "甲", hasLyrics: true, durationMs: 151000, coverUrl: "" },
    { id: "b", title: "Moon", artist: "乙", hasLyrics: false, durationMs: 61000, coverUrl: "" },
    { id: "c", title: "Another MOON", artist: "丙", hasLyrics: true, durationMs: null, coverUrl: "" },
];

function setup(overrides: Partial<QueuePanelProps> = {}) {
    const props: QueuePanelProps = {
        tracks,
        currentId: "b",
        playing: true,
        open: true,
        onPlay: vi.fn(),
        onRemove: vi.fn(),
        onMove: vi.fn(),
        onImport: vi.fn(),
        onClose: vi.fn(),
        ...overrides,
    };
    const result = render(<QueuePanel {...props} />);
    return { ...result, props };
}

afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
});

test("shows the queue and toggles between cover and compact rows", () => {
    const { container } = setup();
    expect(screen.getByText("播放队列")).not.toBeNull();
    expect(screen.getByText("共3首")).not.toBeNull();
    expect(screen.getByText("02:31")).not.toBeNull();
    const current = screen.getByText("Moon").closest(".q-row")!;
    expect(current.classList.contains("on")).toBe(true);
    expect(current.querySelector(".eq")).not.toBeNull();
    expect(container.querySelectorAll(".q-row .cover")).toHaveLength(3);
    const cover = container.querySelector(".q-row .cover");
    fireEvent.click(screen.getByRole("button", { name: "紧凑列表" }));
    expect(container.querySelector(".q-list")!.classList.contains("compact")).toBe(true);
    expect(screen.getByText("Moon - 乙")).not.toBeNull();
    // 紧凑模式只用样式隐藏封面；同一个图片元素保留，切回时不重新加载
    fireEvent.click(screen.getByRole("button", { name: "显示封面" }));
    expect(container.querySelector(".q-row .cover")).toBe(cover);
});

test("scrolls to a located song once, not again on later playback or view changes", () => {
    const scrollTo = vi.fn();
    Object.defineProperty(HTMLElement.prototype, "scrollTo", { value: scrollTo, configurable: true });
    const { props, rerender } = setup({ locateRequest: { id: "c", version: 1 } });
    expect(scrollTo).toHaveBeenCalledTimes(1);
    rerender(<QueuePanel {...props} locateRequest={{ id: "c", version: 1 }} currentId="a" tracks={[...tracks]} />);
    fireEvent.click(screen.getByRole("button", { name: "紧凑列表" }));
    expect(scrollTo).toHaveBeenCalledTimes(1);
    rerender(<QueuePanel {...props} locateRequest={{ id: "a", version: 2 }} />);
    expect(scrollTo).toHaveBeenCalledTimes(2);
    delete (HTMLElement.prototype as { scrollTo?: unknown }).scrollTo;
});

test("searches and highlights case insensitively; clear, escape and actions work", () => {
    const { props, container } = setup();
    fireEvent.click(screen.getByRole("button", { name: "搜索" }));
    const input = screen.getByRole("textbox", { name: "搜索歌名或歌手" });
    fireEvent.change(input, { target: { value: "moon" } });
    expect(screen.getByText("找到 2 首")).not.toBeNull();
    expect(container.querySelectorAll("mark")).toHaveLength(2);
    fireEvent.keyDown(screen.getByText("Moon").closest(".q-row")!, { key: "ArrowDown", altKey: true });
    expect(props.onMove).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "清空搜索" }));
    expect((input as HTMLInputElement).value).toBe("");
    fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByRole("textbox", { name: "搜索歌名或歌手" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "导入音乐和歌词" }));
    fireEvent.click(screen.getByRole("button", { name: "收起播放队列" }));
    expect(props.onImport).toHaveBeenCalledOnce();
    expect(props.onClose).toHaveBeenCalledOnce();
});

test("plays and keyboard-reorders the selected row", () => {
    const { props } = setup();
    const row = screen.getByText("Moon").closest(".q-row")!;
    fireEvent.doubleClick(row);
    fireEvent.keyDown(row, { key: "Enter" });
    expect(props.onPlay).toHaveBeenCalledTimes(2);
    expect(props.onPlay).toHaveBeenCalledWith("b");
    fireEvent.keyDown(row, { key: "ArrowUp", altKey: true });
    expect(props.onMove).toHaveBeenCalledWith(1, 0);
});

test("drags only beyond the threshold, shifts rows, and commits the new position", () => {
    const { props, container } = setup();
    const rows = container.querySelectorAll<HTMLElement>(".q-row");
    const list = container.querySelector<HTMLElement>(".q-list")!;
    for (const row of rows) Object.defineProperty(row, "offsetHeight", { value: 76 });
    list.getBoundingClientRect = () => ({ top: 0, bottom: 400 }) as DOMRect;
    fireEvent.pointerDown(rows[0], { button: 0, clientY: 100 });
    fireEvent.pointerMove(window, { clientY: 103 });
    expect(rows[0].classList.contains("dragging")).toBe(false);
    fireEvent.pointerMove(window, { clientY: 180 });
    expect(rows[0].classList.contains("dragging")).toBe(true);
    expect(rows[1].style.transform).toBe("translateY(-76px)");
    fireEvent.pointerUp(window, { clientY: 180 });
    expect(props.onMove).toHaveBeenCalledWith(0, 1);
});

test("scrolls the queue while dragging near an edge", () => {
    let frame: FrameRequestCallback | undefined;
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
        frame = callback;
        return 1;
    });
    vi.stubGlobal("cancelAnimationFrame", vi.fn());
    const { container } = setup();
    const row = container.querySelector<HTMLElement>(".q-row")!;
    const list = container.querySelector<HTMLElement>(".q-list")!;
    Object.defineProperty(row, "offsetHeight", { value: 76 });
    list.getBoundingClientRect = () => ({ top: 0, bottom: 200 }) as DOMRect;
    fireEvent.pointerDown(row, { button: 0, clientY: 100 });
    fireEvent.pointerMove(window, { clientY: 195 });
    expect(frame).toBeDefined();
    actFrame(frame!);
    expect(list.scrollTop).toBeGreaterThan(0);
    fireEvent.pointerUp(window);
});

function actFrame(callback: FrameRequestCallback) {
    callback(0);
}

test("removes a row with the hover button; pressing it neither drags nor plays", () => {
    const { props } = setup();
    const row = screen.getByText("Moon").closest(".q-row")!;
    const del = row.querySelector<HTMLButtonElement>(".q-del")!;
    expect(row.querySelector(".dur")?.textContent).toBe("01:01");
    fireEvent.pointerDown(del, { button: 0, clientY: 100 });
    fireEvent.pointerMove(window, { clientY: 180 });
    fireEvent.pointerUp(window, { clientY: 180 });
    expect(props.onMove).not.toHaveBeenCalled();
    fireEvent.doubleClick(del);
    expect(props.onPlay).not.toHaveBeenCalled();
    fireEvent.click(del);
    expect(props.onRemove).toHaveBeenCalledExactlyOnceWith("b");
});

test("searching still offers removal for the filtered rows", () => {
    const { props } = setup();
    fireEvent.click(screen.getByRole("button", { name: "搜索" }));
    fireEvent.change(screen.getByRole("textbox", { name: "搜索歌名或歌手" }), { target: { value: "moon" } });
    fireEvent.click(screen.getByText("Moon").closest(".q-row")!.querySelector<HTMLButtonElement>(".q-del")!);
    expect(props.onRemove).toHaveBeenCalledExactlyOnceWith("b");
});

test("Delete removes the focused row and focus lands on the row after it", () => {
    const onRemove = vi.fn();
    function Harness() {
        const [list, setList] = useState(tracks);
        const base: QueuePanelProps = {
            tracks: list,
            currentId: "b",
            playing: true,
            open: true,
            onPlay: vi.fn(),
            onRemove: id => {
                onRemove(id);
                setList(previous => previous.filter(track => track.id !== id));
            },
            onMove: vi.fn(),
            onImport: vi.fn(),
            onClose: vi.fn(),
        };
        return <QueuePanel {...base} />;
    }
    const { container } = render(<Harness />);
    const row = container.querySelector<HTMLElement>('.q-row[data-id="b"]')!;
    row.focus();
    expect(document.activeElement).toBe(row);
    fireEvent.keyDown(row, { key: "Delete" });
    expect(onRemove).toHaveBeenCalledExactlyOnceWith("b");
    const rows = container.querySelectorAll(".q-row");
    expect(rows).toHaveLength(2);
    expect(document.activeElement).toBe(rows[1]);
    expect(rows[1].getAttribute("data-id")).toBe("c");
});

test("Delete on the last row moves focus to the new last row", () => {
    const onRemove = vi.fn();
    function Harness() {
        const [list, setList] = useState(tracks);
        const base: QueuePanelProps = {
            tracks: list,
            currentId: null,
            playing: true,
            open: true,
            onPlay: vi.fn(),
            onRemove: id => {
                onRemove(id);
                setList(previous => previous.filter(track => track.id !== id));
            },
            onMove: vi.fn(),
            onImport: vi.fn(),
            onClose: vi.fn(),
        };
        return <QueuePanel {...base} />;
    }
    const { container } = render(<Harness />);
    const row = container.querySelector<HTMLElement>('.q-row[data-id="c"]')!;
    row.focus();
    fireEvent.keyDown(row, { key: "Delete" });
    expect(onRemove).toHaveBeenCalledExactlyOnceWith("c");
    const rows = container.querySelectorAll(".q-row");
    expect(rows).toHaveLength(2);
    expect(document.activeElement).toBe(rows[1]);
});
