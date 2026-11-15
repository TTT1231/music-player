import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { act } from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { PlayerState } from "../ipc";
import { DesktopLayout } from "./DesktopLayout";

const mocks = vi.hoisted(() => {
    const command = () => vi.fn().mockResolvedValue(undefined);
    return {
        state: null as unknown,
        notice: null as unknown,
        commands: {
            play: command(),
            pause: command(),
            toggle: command(),
            next: command(),
            prev: command(),
            seek: command(),
            setMode: command(),
            setVolume: command(),
            setRate: command(),
            setPitch: command(),
            playTrack: command(),
            removeTrack: command(),
            moveTrack: command(),
            toggleDesktopLyrics: vi.fn().mockResolvedValue(true),
            getLyrics: vi.fn().mockResolvedValue({ lines: [{ timeMs: 1_000, text: "第一句" }] }),
        },
        window: { minimize: command(), toggleMaximize: command(), close: command() },
        webview: { listener: null as null | ((event: { payload: unknown }) => void), unlisten: vi.fn() },
    };
});
vi.mock("../store", () => ({
    usePlayerState: () => ({ state: mocks.state, notice: mocks.notice, error: null, platform: "desktop" }),
    playerStore: { dismissNotice: vi.fn() },
}));
vi.mock("../ipc", () => ({ commands: mocks.commands }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => mocks.window }));
vi.mock("@tauri-apps/api/webview", () => ({
    getCurrentWebview: () => ({
        onDragDropEvent: async (handler: typeof mocks.webview.listener) => {
            mocks.webview.listener = handler;
            return mocks.webview.unlisten;
        },
    }),
}));
vi.mock("./QueuePanel", () => ({
    QueuePanel: ({
        open,
        onClose,
        onImport,
        onRemove,
    }: {
        open: boolean;
        onClose: () => void;
        onImport: () => void;
        onRemove: (id: string) => void;
    }) => (
        <aside aria-label="播放队列" hidden={!open}>
            <button onClick={onClose}>收起播放队列</button>
            <button onClick={onImport}>导入音乐和歌词</button>
            <button onClick={() => onRemove("track")}>从播放列表移除</button>
        </aside>
    ),
}));
vi.mock("./ImportDialog", () => ({
    ImportDialog: ({
        onClose,
        droppedPaths,
        dragging,
    }: {
        onClose: () => void;
        droppedPaths?: string[] | null;
        dragging?: boolean;
    }) => (
        <div
            role="dialog"
            aria-label="导入音乐"
            data-dragging={dragging ? "true" : "false"}
            data-paths={droppedPaths?.join("\n") ?? ""}
        >
            <button onClick={onClose}>关闭导入</button>
        </div>
    ),
}));

const track = {
    id: "track",
    title: "歌曲",
    artist: "歌手",
    hasLyrics: true,
    durationMs: 120_000,
    coverUrl: "http://cover.localhost/track",
};
const state: PlayerState = {
    tracks: [track],
    currentId: track.id,
    playing: false,
    mode: "loop",
    volume: 0.8,
    rate: 1,
    pitch: 0,
    positionMs: 10_000,
    durationMs: 120_000,
};
beforeEach(() => {
    vi.clearAllMocks();
    mocks.state = state;
    mocks.notice = null;
    mocks.webview.listener = null;
});
afterEach(cleanup);

test("展示当前歌曲、歌词和底栏，并控制播放", async () => {
    render(<DesktopLayout />);
    expect(screen.getByRole("heading", { name: "歌曲" })).toBeTruthy();
    expect(screen.getAllByText("歌手").length).toBeGreaterThan(0);
    expect(await screen.findByText("第一句")).toBeTruthy();
    expect(screen.getByText("00:10")).toBeTruthy();
    expect(screen.getByText("02:00")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "播放" }));
    expect(mocks.commands.toggle).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "下一首" }));
    expect(mocks.commands.next).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "桌面歌词" }));
    await waitFor(() => expect(mocks.commands.toggleDesktopLyrics).toHaveBeenCalledOnce());
});

test("队列可收起和展开，歌词随之居中", () => {
    const { container } = render(<DesktopLayout />);
    fireEvent.click(screen.getByRole("button", { name: "收起播放队列" }));
    expect(screen.queryByRole("complementary", { name: "播放队列" })).toBeNull();
    expect(container.querySelector(".lyrics.center")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "播放队列" }));
    expect(screen.getByRole("complementary", { name: "播放队列" })).toBeTruthy();
});

test("模式、音效和音量弹层可操作并可用 Esc 关闭", () => {
    render(<DesktopLayout />);
    fireEvent.click(screen.getByRole("button", { name: "播放模式：列表循环" }));
    fireEvent.click(screen.getByRole("menuitemradio", { name: "随机播放" }));
    expect(mocks.commands.setMode).toHaveBeenCalledWith("shuffle");
    fireEvent.click(screen.getByRole("button", { name: "音效：升降调和倍速" }));
    expect(screen.getByRole("slider", { name: "升降调" })).toBeTruthy();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("slider", { name: "升降调" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "音量" }));
    const slider = screen.getByRole("slider", { name: "音量" });
    expect(slider.getAttribute("aria-valuenow")).toBe("80");
    expect(screen.getByText("80%")).toBeTruthy();
    fireEvent.keyDown(slider, { key: "ArrowDown" });
    expect(mocks.commands.setVolume).toHaveBeenCalledWith(0.75);
});

test("空格只在非输入控件触发播放，导入对话框可用 Esc 关闭", () => {
    render(<DesktopLayout />);
    fireEvent.keyDown(document.body, { code: "Space", key: " " });
    expect(mocks.commands.toggle).toHaveBeenCalledOnce();
    fireEvent.keyDown(screen.getByRole("button", { name: "播放" }), { code: "Space", key: " " });
    expect(mocks.commands.toggle).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "导入音乐和歌词" }));
    expect(screen.getByRole("dialog", { name: "导入音乐" })).toBeTruthy();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "导入音乐" })).toBeNull();
});

test("无当前歌曲时显示原型空状态", () => {
    mocks.state = { ...state, tracks: [], currentId: null };
    render(<DesktopLayout />);
    expect(screen.getByText("选一首歌开始")).toBeTruthy();
    expect(screen.getByText("在右侧播放队列里双击歌曲")).toBeTruthy();
});

test("从播放列表移除后显示轻提示", async () => {
    render(<DesktopLayout />);
    fireEvent.click(screen.getByRole("button", { name: "从播放列表移除" }));
    await waitFor(() => expect(mocks.commands.removeTrack).toHaveBeenCalledExactlyOnceWith("track"));
    expect(await screen.findByText("已从播放列表移除")).toBeTruthy();
});

test("无法读取的文件被跳过时显示轻提示", () => {
    mocks.notice = { kind: "unreadable", title: "损坏.mp3" };
    render(<DesktopLayout />);
    expect(screen.getByText("无法读取 损坏.mp3，已跳过")).toBeTruthy();
});

test("拖入文件即打开导入对话框，放下后导入拖入的路径", async () => {
    render(<DesktopLayout />);
    await waitFor(() => expect(mocks.webview.listener).toBeTypeOf("function"));
    const dialog = () => screen.getByRole("dialog", { name: "导入音乐" });
    expect(screen.queryByRole("dialog")).toBeNull();
    act(() => mocks.webview.listener!({ payload: { type: "enter", position: { x: 10, y: 10 } } }));
    expect(dialog().getAttribute("data-dragging")).toBe("true");
    act(() => mocks.webview.listener!({ payload: { type: "leave" } }));
    expect(dialog().getAttribute("data-dragging")).toBe("false");
    act(() =>
        mocks.webview.listener!({ payload: { type: "drop", position: { x: 10, y: 10 }, paths: ["a.mp3", "a.lrc"] } }),
    );
    expect(dialog().getAttribute("data-dragging")).toBe("false");
    expect(dialog().getAttribute("data-paths")).toBe("a.mp3\na.lrc");
});

test("移除提示之后，新的无法读取提示仍能显示", async () => {
    const view = render(<DesktopLayout />);
    fireEvent.click(screen.getByRole("button", { name: "从播放列表移除" }));
    expect(await screen.findByText("已从播放列表移除")).toBeTruthy();
    mocks.notice = { kind: "unreadable", title: "损坏.mp3" };
    view.rerender(<DesktopLayout />);
    expect(await screen.findByText("无法读取 损坏.mp3，已跳过")).toBeTruthy();
    expect(screen.queryByText("已从播放列表移除")).toBeNull();
});

test("弹层超出底栏时向内收回，与底栏两端保持 8px", () => {
    const rect = (left: number, width: number) =>
        ({
            left,
            right: left + width,
            width,
            top: 0,
            bottom: 40,
            height: 40,
            x: left,
            y: 0,
            toJSON: () => ({}),
        }) as DOMRect;
    const original = Element.prototype.getBoundingClientRect;
    Element.prototype.getBoundingClientRect = function (this: Element) {
        if (this.classList.contains("desktop-bar")) return rect(0, 1000);
        if (this.classList.contains("desktop-pop")) {
            // 未收回时贴着最右侧的音量按钮居中，右边超出底栏 10px
            const shift = parseFloat((this as HTMLElement).style.getPropertyValue("--pop-shift") || "0");
            return rect(958 + shift, 52);
        }
        return original.call(this);
    };
    try {
        render(<DesktopLayout />);
        fireEvent.click(screen.getByRole("button", { name: "音量" }));
        const pop = screen.getByRole("dialog", { name: "调节音量" });
        expect(pop.style.getPropertyValue("--pop-shift")).toBe("-18px");
    } finally {
        Element.prototype.getBoundingClientRect = original;
    }
});
