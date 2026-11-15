import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { useState } from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { ImportDialog } from "./ImportDialog";
import { useNativeImportDrop } from "./useNativeImportDrop";

type DropPayload =
    | { type: "enter" | "over"; position: { x: number; y: number } }
    | { type: "drop"; position: { x: number; y: number }; paths: string[] }
    | { type: "leave" };
const native = vi.hoisted(() => ({
    listener: null as null | ((event: { payload: DropPayload }) => void),
    unlisten: vi.fn(),
}));
vi.mock("@tauri-apps/api/webview", () => ({
    getCurrentWebview: () => ({
        onDragDropEvent: async (handler: typeof native.listener) => {
            native.listener = handler;
            return native.unlisten;
        },
    }),
}));

const props = () => ({ onClose: vi.fn(), onImported: vi.fn(), onGoListen: vi.fn() });
beforeEach(() => {
    native.listener = null;
    native.unlisten.mockClear();
});
afterEach(() => {
    cleanup();
    clearMocks();
    vi.unstubAllGlobals();
});

test("selects music and shows a single primary action that locates the first imported track", async () => {
    const events: Array<[string, unknown]> = [];
    mockIPC((command, args) => {
        events.push([command, args]);
        return { added: ["first", "second"], withLyrics: 1, lyricsAttached: 0, orphanLyrics: 0 };
    });
    const handlers = props();
    render(<ImportDialog {...handlers} />);
    expect(screen.getByText("把歌曲拖到这里")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "选择歌曲" }));
    expect((await screen.findByText(/已导入/)).textContent).toBe("已导入 2 首，1 首有歌词");
    expect(events).toEqual([["pick_and_import", { kind: "files" }]]);
    expect(handlers.onImported).toHaveBeenCalledWith(["first", "second"]);
    expect(document.querySelectorAll(".import-button.primary")).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "去听歌" }));
    expect(handlers.onGoListen).toHaveBeenCalledWith("first");
    expect(handlers.onClose).toHaveBeenCalledOnce();
});

test("canceling folder selection leaves the dialog ready; lyrics-only reports the attachment", async () => {
    let attempt = 0;
    mockIPC(() => (++attempt === 1 ? null : { added: [], withLyrics: 0, lyricsAttached: 2, orphanLyrics: 0 }));
    const handlers = props();
    render(<ImportDialog {...handlers} />);
    fireEvent.click(screen.getByRole("button", { name: "导入整个文件夹" }));
    await waitFor(() => expect(attempt).toBe(1));
    expect(screen.queryByRole("status")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "导入整个文件夹" }));
    expect((await screen.findByText(/已为/)).textContent).toBe("已为 2 首歌加上歌词");
    fireEvent.click(screen.getByRole("button", { name: "去听歌" }));
    expect(handlers.onGoListen).toHaveBeenCalledWith(null);
});

test("no matching tracks leave the selection button as the primary action", async () => {
    mockIPC(() => ({ added: [], withLyrics: 0, lyricsAttached: 0, orphanLyrics: 1 }));
    render(<ImportDialog {...props()} />);
    fireEvent.click(screen.getByRole("button", { name: "选择歌曲" }));
    expect(await screen.findByText("没有找到歌曲，请重新选择")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "去听歌" })).toBeNull();
    expect(screen.getByRole("button", { name: "选择歌曲" }).classList.contains("primary")).toBe(true);
});

test("closes with Escape, the close button, or a backdrop click", () => {
    const handlers = props();
    const { unmount } = render(<ImportDialog {...handlers} />);
    fireEvent.keyDown(document, { key: "Escape" });
    fireEvent.click(screen.getByRole("button", { name: "关闭" }));
    fireEvent.click(screen.getByTestId("import-backdrop"));
    expect(handlers.onClose).toHaveBeenCalledTimes(3);
    unmount();
});

test("native file drag opens the dialog anywhere and imports dropped paths exactly once", async () => {
    const calls: unknown[] = [];
    mockIPC((command, args) => {
        calls.push([command, args]);
        return { added: ["song"], withLyrics: 0, lyricsAttached: 0, orphanLyrics: 0 };
    });
    const handlers = props();
    function Harness() {
        const [open, setOpen] = useState(false);
        const [dragging, setDragging] = useState(false);
        const [paths, setPaths] = useState<string[] | null>(null);
        useNativeImportDrop({
            onEnter: () => {
                setOpen(true);
                setDragging(true);
            },
            onLeave: () => setDragging(false),
            onDrop: value => {
                setOpen(true);
                setDragging(false);
                setPaths(value);
            },
        });
        return open ? <ImportDialog {...handlers} droppedPaths={paths} dragging={dragging} /> : null;
    }
    const { unmount } = render(<Harness />);
    await waitFor(() => expect(native.listener).toBeTypeOf("function"));
    act(() => native.listener?.({ payload: { type: "enter", position: { x: 10, y: 10 } } }));
    expect(screen.getByRole("dialog")).toBeTruthy();
    act(() =>
        native.listener?.({ payload: { type: "drop", position: { x: 10, y: 10 }, paths: ["song.mp3", "song.lrc"] } }),
    );
    expect((await screen.findByText(/已导入/)).textContent).toBe("已导入 1 首");
    expect(calls).toEqual([["import_paths", { paths: ["song.mp3", "song.lrc"] }]]);
    expect(handlers.onImported).toHaveBeenCalledWith(["song"]);
    unmount();
    expect(native.unlisten).toHaveBeenCalledOnce();
});
