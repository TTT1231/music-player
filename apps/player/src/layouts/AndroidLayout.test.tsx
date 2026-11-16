import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { PlayerState } from "../ipc";
import { AndroidLayout } from "./AndroidLayout";

const mocks = vi.hoisted(() => {
    const command = () => vi.fn().mockResolvedValue(undefined);
    return {
        back: null as null | (() => void),
        state: null as unknown,
        commands: {
            play: command(),
            pause: command(),
            toggle: command(),
            next: command(),
            prev: command(),
            setMode: command(),
            setVolume: command(),
            setRate: command().mockResolvedValue(1.2),
            setPitch: command(),
            playTrack: command(),
            removeTrack: command(),
            pickAndImport: command().mockResolvedValue(null),
            notifyState: command(),
            moveToBackground: command(),
            startPlaybackEvents: command(),
            getLyrics: vi.fn().mockResolvedValue({ lines: [{ timeMs: 1_000, text: "第一句" }] }),
        },
    };
});
vi.mock("../store", () => ({
    usePlayerState: () => ({ state: mocks.state, notice: null, error: null, platform: "android" }),
    usePlaybackPosition: () => (mocks.state as { positionMs?: number } | null)?.positionMs ?? 0,
    playerStore: { dismissNotice: vi.fn() },
}));
vi.mock("../ipc", async importOriginal => {
    const actual = await importOriginal<typeof import("../ipc")>();
    return {
        ...actual,
        commands: mocks.commands,
        listenPlaybackEvents: vi.fn().mockResolvedValue(() => {}),
        onBackButton: vi.fn(async (handler: () => void) => {
            mocks.back = handler;
            return () => {};
        }),
    };
});

const tracks = [
    { id: "a", title: "春天", artist: "甲", hasLyrics: true, durationMs: 151_000, coverUrl: "" },
    { id: "b", title: "Moon", artist: "乙", hasLyrics: false, durationMs: 61_000, coverUrl: "" },
];
const state: PlayerState = {
    tracks: tracks as PlayerState["tracks"],
    currentId: "b",
    playing: true,
    mode: "loop",
    volume: 0.8,
    rate: 1,
    pitch: 0,
    positionMs: 10_000,
    durationMs: 61_000,
};

beforeEach(() => {
    vi.clearAllMocks();
    mocks.back = null;
    mocks.state = state;
});
afterEach(cleanup);

test("首页即播放列表：右上角搜索与 +，每行末尾 ×，正在播放行强调色与均衡条", async () => {
    const { container } = render(<AndroidLayout />);
    expect(await screen.findByText("春天")).toBeTruthy();
    expect(screen.getByRole("button", { name: "导入音乐和歌词" })).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "搜索歌名或歌手" })).toBeTruthy();
    // 列表行与迷你条都会出现当前歌名；取列表里的那行
    const currentRow = container.querySelector(".m-row.on")!;
    expect(currentRow.textContent).toContain("Moon");
    expect(currentRow.querySelector(".eq")).not.toBeNull();
    // 移除当前行
    fireEvent.click(screen.getByRole("button", { name: "从播放列表移除：Moon" }));
    await waitFor(() => expect(mocks.commands.removeTrack).toHaveBeenCalledWith("b"));
    // 迷你条显示当前歌曲与进度
    expect(container.querySelector(".mini .t")?.textContent).toBe("Moon");
});

test("点行播放并进入播放页，控制按钮无文字且收起可回列表", async () => {
    render(<AndroidLayout />);
    fireEvent.click(await screen.findByText("春天"));
    await waitFor(() => expect(mocks.commands.playTrack).toHaveBeenCalledWith("a"));
    // 播放页：模式、上一首、播放、下一首、列表（无文字，仅 aria-label）
    const controls = ["上一首", "下一首", "播放列表"];
    for (const label of controls) expect(screen.getByRole("button", { name: label })).toBeTruthy();
    expect(screen.getByRole("button", { name: "暂停" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "收起" })).toBeTruthy();
    // 收起回列表
    fireEvent.click(screen.getByRole("button", { name: "收起" }));
    expect(screen.getByRole("textbox", { name: "搜索歌名或歌手" })).toBeTruthy();
});

test("切换播放模式显示中间偏下的深色轻提示", async () => {
    render(<AndroidLayout />);
    fireEvent.click(await screen.findByText("春天"));
    vi.useFakeTimers();
    try {
        fireEvent.click(screen.getByRole("button", { name: "播放模式：列表循环" }));
        await act(async () => {}); // 冲刷微任务：setMode 成功后弹出提示
        const toast = screen.getByRole("status");
        expect(toast.textContent).toContain("随机播放");
        expect(toast.className).toContain("player-toast--mobile");
        // 1.8 秒后自动消失
        act(() => vi.advanceTimersByTime(2_000));
        expect(screen.queryByRole("status")).toBeNull();
    } finally {
        vi.useRealTimers();
    }
});

test("⋮ 打开升降调/倍速面板（无恢复正常）；倍速非 1.0 显示 1.2X 角标", async () => {
    mocks.state = { ...state, rate: 1.2 };
    render(<AndroidLayout />);
    fireEvent.click(await screen.findByText("春天"));
    expect(screen.getByText("1.2X")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "升降调和倍速" }));
    expect(screen.getByRole("slider", { name: "升降调" })).toBeTruthy();
    expect(screen.getByRole("slider", { name: "倍速" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "恢复正常" })).toBeNull();
});

test("播放列表按钮打开底部弹层；点弹层外关闭；弹层行有 ×", async () => {
    render(<AndroidLayout />);
    fireEvent.click(await screen.findByText("春天"));
    fireEvent.click(screen.getByRole("button", { name: "播放列表" }));
    const dialog = await screen.findByRole("dialog", { name: "播放列表" });
    expect(dialog.textContent).toContain("播放列表 (2)");
    expect(screen.getAllByRole("button", { name: "从播放列表移除：春天" })).toBeTruthy();
    // 点弹层外（scrim）关闭
    fireEvent.click(screen.getByTestId("scrim"));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "播放列表" })).toBeNull());
});

test("导入页：返回、图标、导入音乐、选择歌曲主按钮、导入整个文件夹，无说明文字", async () => {
    render(<AndroidLayout />);
    fireEvent.click(await screen.findByRole("button", { name: "导入音乐和歌词" }));
    expect(screen.getByRole("heading", { name: "导入音乐" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "选择歌曲" }).classList.contains("primary")).toBe(true);
    expect(screen.getByRole("button", { name: "导入整个文件夹" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "返回" })).toBeTruthy();
    // 无说明文字：页面没有页面标题以外的说明段落
    expect(screen.queryByText(/歌词文件和歌曲一起选/)).toBeNull();
    // 返回回列表
    fireEvent.click(screen.getByRole("button", { name: "返回" }));
    expect(screen.getByRole("textbox", { name: "搜索歌名或歌手" })).toBeTruthy();
});

test("选择歌曲经外壳导入并显示一句结果；去听歌回到列表、不自动播放", async () => {
    mocks.commands.pickAndImport.mockResolvedValueOnce({
        added: ["a"],
        withLyrics: 1,
        lyricsAttached: 0,
        orphanLyrics: 0,
    });
    render(<AndroidLayout />);
    fireEvent.click(await screen.findByRole("button", { name: "导入音乐和歌词" }));
    fireEvent.click(screen.getByRole("button", { name: "选择歌曲" }));
    expect(mocks.commands.pickAndImport).toHaveBeenCalledWith("files");
    expect(await screen.findByText("已导入 1 首，1 首有歌词")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "去听歌" }));
    expect(mocks.commands.playTrack).not.toHaveBeenCalled();
    expect(screen.queryByRole("heading", { name: "导入音乐" })).toBeNull();
});

test("取消文件夹选择时不显示结果", async () => {
    render(<AndroidLayout />);
    fireEvent.click(await screen.findByRole("button", { name: "导入音乐和歌词" }));
    fireEvent.click(screen.getByRole("button", { name: "导入整个文件夹" }));
    expect(mocks.commands.pickAndImport).toHaveBeenCalledWith("folder");
    await waitFor(() =>
        expect((screen.getByRole("button", { name: "导入整个文件夹" }) as HTMLButtonElement).disabled).toBe(false),
    );
    expect(screen.queryByRole("status")).toBeNull();
});

test("通知栏和锁屏卡片收到时长、位置和倍速，系统才能显示进度", async () => {
    render(<AndroidLayout />);
    await waitFor(() =>
        expect(mocks.commands.notifyState).toHaveBeenCalledWith({
            playing: true,
            title: "Moon",
            artist: "乙",
            durationMs: 61_000,
            positionMs: 10_000,
            rate: 1,
        }),
    );
});

test("系统返回键：先关弹层，再从播放页回列表，在列表时退到后台而不是关闭应用", async () => {
    render(<AndroidLayout />);
    fireEvent.click(await screen.findByText("春天"));
    fireEvent.click(screen.getByRole("button", { name: "播放列表" }));
    await screen.findByRole("dialog", { name: "播放列表" });
    await waitFor(() => expect(mocks.back).not.toBeNull());
    act(() => mocks.back!());
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "播放列表" })).toBeNull());
    expect(screen.getByRole("button", { name: "收起" })).toBeTruthy();
    act(() => mocks.back!());
    expect(screen.getByRole("textbox", { name: "搜索歌名或歌手" })).toBeTruthy();
    expect(mocks.commands.moveToBackground).not.toHaveBeenCalled();
    act(() => mocks.back!());
    expect(mocks.commands.moveToBackground).toHaveBeenCalledOnce();
});

test("系统返回键在导入页回到列表", async () => {
    render(<AndroidLayout />);
    fireEvent.click(await screen.findByRole("button", { name: "导入音乐和歌词" }));
    await waitFor(() => expect(mocks.back).not.toBeNull());
    act(() => mocks.back!());
    expect(screen.getByRole("textbox", { name: "搜索歌名或歌手" })).toBeTruthy();
    expect(mocks.commands.moveToBackground).not.toHaveBeenCalled();
});
