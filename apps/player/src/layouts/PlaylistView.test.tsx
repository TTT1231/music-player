import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { SavedPlaylist } from "../ipc";
import { PlaylistView } from "./PlaylistView";

const tracks = [
    { id: "a", title: "第一首", artist: "歌手", durationMs: 10000, hasLyrics: false, coverUrl: "" },
    { id: "b", title: "第二首", artist: "歌手", durationMs: 20000, hasLyrics: false, coverUrl: "" },
];
afterEach(cleanup);
const setup = (playlists: SavedPlaylist[] = [{ id: "list-1", name: "收藏", trackIds: ["a"] }]) => {
    const props = {
        library: tracks,
        playlists,
        currentId: null,
        playing: false,
        onCreate: vi.fn().mockResolvedValue("list-2"),
        onAdd: vi.fn().mockResolvedValue(undefined),
        onDelete: vi.fn().mockResolvedValue(undefined),
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

test("右键删除指定的自建歌单，取消时保留歌单，全部音乐没有删除选项", async () => {
    const props = setup([
        { id: "list-1", name: "收藏", trackIds: ["a"] },
        { id: "list-2", name: "通勤", trackIds: ["b"] },
    ]);
    fireEvent.click(screen.getByRole("button", { name: /收藏/ }));
    const commuting = screen.getByRole("button", { name: /通勤/ });
    fireEvent.contextMenu(commuting);
    fireEvent.contextMenu(screen.getByRole("button", { name: /全部音乐/ }));
    expect(screen.queryByRole("menu")).toBeNull();
    expect(props.onDelete).not.toHaveBeenCalled();

    fireEvent.contextMenu(commuting);
    fireEvent.click(screen.getByRole("menuitem", { name: "删除歌单「通勤」" }));
    const dialog = screen.getByRole("alertdialog", { name: "删除歌单" });
    expect(within(dialog).getByText(/删除「通勤」/)).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "取消" }));
    expect(props.onDelete).not.toHaveBeenCalled();

    fireEvent.contextMenu(commuting);
    fireEvent.click(screen.getByRole("menuitem", { name: "删除歌单「通勤」" }));
    fireEvent.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "删除" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(props.onDelete).toHaveBeenCalledExactlyOnceWith("list-2");
    expect(screen.getByRole("heading", { name: "收藏" })).toBeTruthy();
});

test("键盘可打开和关闭菜单，删除当前歌单后回到全部音乐", async () => {
    const props = setup();
    const favorite = screen.getByRole("button", { name: /收藏/ });
    fireEvent.click(favorite);
    favorite.focus();
    fireEvent.keyDown(favorite, { key: "F10", shiftKey: true });
    const deletion = screen.getByRole("menuitem", { name: "删除歌单「收藏」" });
    expect(document.activeElement).toBe(deletion);
    fireEvent.keyDown(deletion, { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();
    expect(document.activeElement).toBe(favorite);

    fireEvent.keyDown(favorite, { key: "ContextMenu" });
    fireEvent.click(screen.getByRole("menuitem", { name: "删除歌单「收藏」" }));
    fireEvent.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "删除" }));
    await screen.findByRole("heading", { name: "全部音乐" });
    fireEvent.click(screen.getByRole("button", { name: "播放全部" }));
    expect(props.onDelete).toHaveBeenCalledExactlyOnceWith("list-1");
    expect(props.onPlay).toHaveBeenCalledWith(null, null);
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
