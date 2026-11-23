import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { PlaylistView } from "./PlaylistView";

const tracks = [
    { id: "a", title: "第一首", artist: "歌手", durationMs: 10000, hasLyrics: false, coverUrl: "" },
    { id: "b", title: "第二首", artist: "歌手", durationMs: 20000, hasLyrics: false, coverUrl: "" },
];
afterEach(cleanup);
const setup = () => {
    const props = {
        library: tracks,
        playlists: [{ id: "list-1", name: "收藏", trackIds: ["a"] }],
        currentId: null,
        playing: false,
        onCreate: vi.fn().mockResolvedValue("list-2"),
        onAdd: vi.fn().mockResolvedValue(undefined),
        onRemove: vi.fn(),
        onPlay: vi.fn(),
        onImport: vi.fn(),
    };
    render(<PlaylistView {...props} />);
    return props;
};

test("创建歌单前检查空名称和重复名称", async () => {
    const props = setup();
    fireEvent.click(screen.getByRole("button", { name: "新建歌单" }));
    const input = screen.getByLabelText("歌单名称");
    fireEvent.change(input, { target: { value: " 收藏 " } });
    expect(screen.getByText("这个名称已被使用。")).toBeTruthy();
    expect((screen.getByRole("button", { name: "创建" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(input, { target: { value: "旅行" } });
    fireEvent.click(screen.getByRole("button", { name: "创建" }));
    expect(props.onCreate).toHaveBeenCalledWith("旅行");
});

test("从曲库选择歌曲、搜索并播放歌单", () => {
    const props = setup();
    fireEvent.click(screen.getByRole("button", { name: /收藏/ }));
    fireEvent.click(screen.getByRole("button", { name: "添加歌曲" }));
    const picker = screen.getByRole("dialog", { name: "添加到「收藏」" });
    expect((within(picker).getByLabelText("选择 第一首") as HTMLInputElement).disabled).toBe(true);
    fireEvent.click(within(picker).getByLabelText("选择 第二首"));
    fireEvent.click(within(picker).getByRole("button", { name: "添加" }));
    expect(props.onAdd).toHaveBeenCalledWith("list-1", ["b"]);
    fireEvent.click(screen.getByRole("button", { name: "播放全部" }));
    expect(props.onPlay).toHaveBeenCalledWith("list-1", null);
    fireEvent.click(screen.getByRole("button", { name: "从当前歌单移出 第一首" }));
    expect(props.onRemove).toHaveBeenCalledWith("list-1", "a");
});
