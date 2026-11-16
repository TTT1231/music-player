import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { PlayerState } from "../ipc";
import { DesktopLyrics } from "./DesktopLyrics";

const mocks = vi.hoisted(() => {
    const command = () => vi.fn().mockResolvedValue(undefined);
    return {
        state: null as unknown,
        commands: {
            toggle: command(),
            next: command(),
            prev: command(),
            toggleDesktopLyrics: vi.fn().mockResolvedValue(false),
            getLyrics: vi.fn().mockResolvedValue({
                lines: [
                    { timeMs: 1_000, text: "第一句" },
                    { timeMs: 3_000, text: "第二句" },
                ],
            }),
        },
    };
});
vi.mock("../store", () => ({
    usePlayerState: () => ({ state: mocks.state, notice: null, error: null, platform: "desktop" }),
    usePlaybackPosition: () => (mocks.state as { positionMs?: number } | null)?.positionMs ?? 0,
}));
vi.mock("../ipc", () => ({ commands: mocks.commands }));

const track = { id: "track", title: "歌曲", artist: "歌手", hasLyrics: true, durationMs: 10_000, coverUrl: "" };
const state: PlayerState = {
    tracks: [track],
    currentId: track.id,
    playing: true,
    mode: "loop",
    volume: 0.8,
    rate: 1,
    pitch: 0,
    positionMs: 2_000,
    durationMs: 10_000,
};

beforeEach(() => {
    vi.clearAllMocks();
    mocks.state = state;
});
afterEach(cleanup);

test("两行交替显示：当前行着色，下一行空闲预告", async () => {
    const { container } = render(<DesktopLyrics />);
    await waitFor(() => expect(container.querySelector(".dl.a .base")?.textContent).toBe("第一句"));
    const a = container.querySelector(".dl.a")!;
    expect(a.classList.contains("idle")).toBe(false);
    const b = container.querySelector(".dl.b")!;
    expect(b.querySelector(".base")?.textContent).toBe("第二句");
    expect(b.classList.contains("idle")).toBe(true);
    expect(a.getAttribute("style")).toContain("--k");
});

test("悬浮工具栏控制播放并可关闭窗口", async () => {
    render(<DesktopLyrics />);
    fireEvent.click(await screen.findByRole("button", { name: "上一首" }));
    expect(mocks.commands.prev).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "暂停" }));
    expect(mocks.commands.toggle).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "下一首" }));
    expect(mocks.commands.next).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "关闭桌面歌词" }));
    await waitFor(() => expect(mocks.commands.toggleDesktopLyrics).toHaveBeenCalledOnce());
});

test("字号按钮在 22–60px 之间调整", async () => {
    const { container } = render(<DesktopLyrics />);
    await waitFor(() => expect(container.querySelector(".dl.a .base")?.textContent).toBe("第一句"));
    const desk = screen.getByTestId("desk");
    expect(desk.style.getPropertyValue("--fs")).toBe("32px");
    fireEvent.click(screen.getByRole("button", { name: "放大字号" }));
    expect(desk.style.getPropertyValue("--fs")).toBe("36px");
    for (let i = 0; i < 10; i++) fireEvent.click(screen.getByRole("button", { name: "放大字号" }));
    expect(desk.style.getPropertyValue("--fs")).toBe("60px");
    fireEvent.click(screen.getByRole("button", { name: "放大字号" }));
    expect(desk.style.getPropertyValue("--fs")).toBe("60px");
    for (let i = 0; i < 12; i++) fireEvent.click(screen.getByRole("button", { name: "缩小字号" }));
    expect(desk.style.getPropertyValue("--fs")).toBe("22px");
});

test("暂停时工具栏显示“播放”，无歌词时居中显示纯音乐提示", async () => {
    const instrumental = {
        id: "inst",
        title: "钢琴曲",
        artist: "作曲家",
        hasLyrics: false,
        durationMs: 5_000,
        coverUrl: "",
    };
    mocks.state = { ...state, playing: false, currentId: instrumental.id, tracks: [track, instrumental] };
    render(<DesktopLyrics />);
    expect(await screen.findByRole("button", { name: "播放" })).toBeTruthy();
    await waitFor(() => expect(document.querySelector(".dl .base")?.textContent).toBe("纯音乐，请欣赏"));
});

test("整个歌词区域都可拖动窗口：行容器带拖动标记，文字层不拦截按下", async () => {
    const { container } = render(<DesktopLyrics />);
    await waitFor(() => expect(container.querySelector(".dl.a .base")?.textContent).toBe("第一句"));
    expect(container.querySelector(".desk-lines")?.hasAttribute("data-tauri-drag-region")).toBe(true);
    for (const line of container.querySelectorAll(".dl")) {
        expect(line.hasAttribute("data-tauri-drag-region")).toBe(true);
    }
});
