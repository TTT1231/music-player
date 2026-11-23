import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { LibraryState, PlayerState } from "../ipc";
import { AndroidLayout } from "./AndroidLayout";

const mocks = vi.hoisted(() => {
    const command = () => vi.fn().mockResolvedValue(undefined);
    return {
        back: null as null | (() => void),
        state: null as unknown,
        library: null as unknown,
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
            playFrom: command().mockResolvedValue(true),
            playPlaylist: command(),
            removeTrack: command(),
            removeQueueTrack: command(),
            createPlaylist: command().mockResolvedValue("p2"),
            renamePlaylist: command(),
            deletePlaylist: command(),
            addPlaylistTracks: command().mockResolvedValue(1),
            removePlaylistTrack: command(),
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
    useLibrary: () => mocks.library,
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

const track = (id: string, title: string, artist: string, hasLyrics: boolean) => ({
    id,
    title,
    artist,
    hasLyrics,
    durationMs: 61_000,
    coverUrl: "",
});
const a = track("a", "春天", "甲", true);
const b = track("b", "Moon", "乙", false);
const c = track("c", "夜曲", "丙", true);
const state: PlayerState = {
    tracks: [a, b],
    currentId: "b",
    playing: true,
    mode: "loop",
    volume: 0.8,
    rate: 1,
    pitch: 0,
    positionMs: 10_000,
    durationMs: 61_000,
};
const library: LibraryState = {
    tracks: [a, b, c],
    playlists: [{ id: "p1", name: "夜跑", trackIds: ["c", "a"] }],
};

const page = (container: HTMLElement, kind: string) => container.querySelector<HTMLElement>(`[data-page="${kind}"]`)!;
const search = (scope: HTMLElement) => within(scope).getByRole("textbox", { name: "搜索歌名或歌手" });
const openAll = (container: HTMLElement) => {
    fireEvent.click(screen.getByText("全部音乐"));
    return page(container, "playlist");
};
const openPlayer = (container: HTMLElement) => {
    fireEvent.click(within(openAll(container)).getByText("春天"));
    return screen.findByRole("button", { name: "收起" });
};

beforeEach(() => {
    vi.clearAllMocks();
    mocks.back = null;
    mocks.state = state;
    mocks.library = library;
});
afterEach(cleanup);

test("首页是歌单列表：内置的「全部音乐」排第一，自建歌单按创建顺序并各显示首数", () => {
    const { container } = render(<AndroidLayout />);
    const home = page(container, "home");
    expect(within(home).getByRole("heading", { name: "歌单" })).toBeTruthy();
    expect(
        within(home)
            .getAllByRole("listitem")
            .map(row => row.querySelector(".pl-tx")?.textContent),
    ).toEqual(["全部音乐3首", "夜跑2首"]);
    expect(within(home).getByRole("button", { name: "新建歌单" })).toBeTruthy();
});

test("迷你条只在有歌在播时出现，点它进入播放页", async () => {
    mocks.state = { ...state, tracks: [], currentId: null, playing: false };
    const view = render(<AndroidLayout />);
    expect(view.container.querySelector(".mini")).toBeNull();
    mocks.state = state;
    view.rerender(<AndroidLayout />);
    expect(view.container.querySelector(".mini .t")?.textContent).toBe("Moon");
    fireEvent.click(within(page(view.container, "home")).getByText("Moon"));
    expect(await screen.findByRole("button", { name: "收起" })).toBeTruthy();
    expect(mocks.commands.playFrom).not.toHaveBeenCalled();
    expect(mocks.commands.playPlaylist).not.toHaveBeenCalled();
});

test("新建歌单：名称不能为空，创建后直接进入这个歌单", async () => {
    render(<AndroidLayout />);
    fireEvent.click(screen.getByRole("button", { name: "新建歌单" }));
    const dialog = screen.getByRole("dialog", { name: "新建歌单" });
    const create = within(dialog).getByRole("button", { name: "创建" }) as HTMLButtonElement;
    expect(create.disabled).toBe(true);
    fireEvent.change(within(dialog).getByRole("textbox", { name: "新建歌单" }), { target: { value: "   " } });
    expect(create.disabled).toBe(true);
    fireEvent.change(within(dialog).getByRole("textbox", { name: "新建歌单" }), { target: { value: " 夜跑 " } });
    fireEvent.click(create);
    await waitFor(() => expect(mocks.commands.createPlaylist).toHaveBeenCalledWith(" 夜跑 "));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "新建歌单" })).toBeNull());
    // p2 是自建歌单：＋ 是添加歌曲，不是导入
    expect(screen.getByRole("button", { name: "添加歌曲到歌单" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "导入音乐和歌词" })).toBeNull();
});

test("歌单重名时提示原因并留在对话框里", async () => {
    mocks.commands.createPlaylist.mockRejectedValueOnce("已经有同名的歌单了");
    render(<AndroidLayout />);
    fireEvent.click(screen.getByRole("button", { name: "新建歌单" }));
    fireEvent.change(screen.getByRole("textbox", { name: "新建歌单" }), { target: { value: "夜跑" } });
    fireEvent.click(screen.getByRole("button", { name: "创建" }));
    expect((await screen.findByRole("status")).textContent).toContain("已经有同名的歌单了");
    expect(screen.getByRole("dialog", { name: "新建歌单" })).toBeTruthy();
});

test("进入「全部音乐」：名称、首数、播放全部、导入和搜索；返回键先清空搜索再回首页", async () => {
    const { container } = render(<AndroidLayout />);
    await waitFor(() => expect(mocks.back).not.toBeNull());
    const list = openAll(container);
    expect(within(list).getByRole("heading", { name: "全部音乐" })).toBeTruthy();
    expect(within(list).getByText("3首")).toBeTruthy();
    expect(within(list).getByRole("button", { name: "播放全部" })).toBeTruthy();
    expect(within(list).getByRole("button", { name: "导入音乐和歌词" })).toBeTruthy();
    expect(within(list).queryByRole("button", { name: "歌单更多操作" })).toBeNull();
    fireEvent.change(search(list), { target: { value: " mo " } });
    expect(within(list).queryByText("春天")).toBeNull();
    expect(within(list).getByText("找到 1 首")).toBeTruthy();
    act(() => mocks.back!());
    expect((search(list) as HTMLInputElement).value).toBe("");
    act(() => mocks.back!());
    expect(screen.getByRole("heading", { name: "歌单" })).toBeTruthy();
    expect(mocks.commands.moveToBackground).not.toHaveBeenCalled();
});

test("没有匹配的歌曲时提示；曲库为空时播放全部不可用并提示去导入", () => {
    mocks.library = { tracks: [], playlists: [] };
    const view = render(<AndroidLayout />);
    const list = openAll(view.container);
    expect(within(list).getByText("还没有音乐。点上面的 + 导入音乐和歌词。")).toBeTruthy();
    expect((within(list).getByRole("button", { name: "播放全部" }) as HTMLButtonElement).disabled).toBe(true);
    mocks.library = library;
    view.rerender(<AndroidLayout />);
    fireEvent.change(search(list), { target: { value: "不存在" } });
    expect(within(list).getByText("没有找到匹配的歌曲。")).toBeTruthy();
    expect((within(list).getByRole("button", { name: "播放全部" }) as HTMLButtonElement).disabled).toBe(true);
});

test("点歌曲：播放列表换成当前列表的顺序并从点中的歌开始，然后进入播放页", async () => {
    const { container } = render(<AndroidLayout />);
    const list = openAll(container);
    fireEvent.click(within(list).getByText("春天"));
    await waitFor(() => expect(mocks.commands.playPlaylist).toHaveBeenCalledWith(null, "a"));
    expect(await screen.findByRole("button", { name: "收起" })).toBeTruthy();
    // 搜索时按搜索结果的顺序
    act(() => mocks.back!());
    await waitFor(() => expect(screen.queryByRole("button", { name: "收起" })).toBeNull());
    fireEvent.change(search(list), { target: { value: "夜" } });
    fireEvent.click(within(list.querySelector(".m-list")!).getByText("夜曲"));
    expect(mocks.commands.playFrom).toHaveBeenLastCalledWith(["c"], "c");
});

test("点的还是正在听的那首：不重新开始也不换播放列表，暂停着就继续播", async () => {
    mocks.state = { ...state, playing: false };
    const { container } = render(<AndroidLayout />);
    const list = openAll(container);
    // 列表行与迷你条都会出现当前歌名，取列表里的那行
    fireEvent.click(within(list.querySelector(".m-list")!).getByText("Moon"));
    expect(mocks.commands.playFrom).not.toHaveBeenCalled();
    expect(mocks.commands.playPlaylist).not.toHaveBeenCalled();
    expect(mocks.commands.play).toHaveBeenCalledOnce();
    expect(await screen.findByRole("button", { name: "收起" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "播放" })).toBeTruthy();
});

test("播放全部从可见的第一首开始；正在播放的行用强调色和均衡条标出", async () => {
    const { container } = render(<AndroidLayout />);
    const list = openAll(container);
    const current = list.querySelector(".m-row.on")!;
    expect(current.textContent).toContain("Moon");
    expect(current.querySelector(".eq")).not.toBeNull();
    fireEvent.click(within(list).getByRole("button", { name: "播放全部" }));
    expect(mocks.commands.playPlaylist).toHaveBeenCalledWith(null, "a");
    expect(await screen.findByRole("button", { name: "收起" })).toBeTruthy();
});

test("自建歌单页：按歌单顺序列歌，＋ 是添加歌曲，⋯ 里有重命名和删除", async () => {
    const { container } = render(<AndroidLayout />);
    fireEvent.click(screen.getByText("夜跑"));
    const list = page(container, "playlist");
    expect(within(list).getByRole("heading", { name: "夜跑" })).toBeTruthy();
    expect(within(list).getByText("2首")).toBeTruthy();
    expect([...list.querySelectorAll(".m-row")].map(row => row.querySelector(".t")?.textContent)).toEqual([
        "夜曲",
        "春天",
    ]);
    expect(within(list).getByRole("button", { name: "添加歌曲到歌单" })).toBeTruthy();
    fireEvent.click(within(list).getByRole("button", { name: "歌单更多操作" }));
    const menu = screen.getByRole("menu", { name: "歌单操作" });
    expect(within(menu).getByRole("menuitem", { name: "重命名" })).toBeTruthy();
    expect(within(menu).getByRole("menuitem", { name: "删除歌单" })).toBeTruthy();
    act(() => mocks.back!());
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
});

test("重命名歌单：带出原名，保存后关对话框；重名时提示并保留对话框", async () => {
    render(<AndroidLayout />);
    fireEvent.click(screen.getByText("夜跑"));
    fireEvent.click(screen.getByRole("button", { name: "歌单更多操作" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "重命名" }));
    await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
    let dialog = screen.getByRole("dialog", { name: "重命名歌单" });
    const input = within(dialog).getByRole("textbox", { name: "重命名歌单" }) as HTMLInputElement;
    expect(input.value).toBe("夜跑");
    mocks.commands.renamePlaylist.mockRejectedValueOnce("已经有同名的歌单了");
    fireEvent.change(input, { target: { value: "白天" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "保存" }));
    expect(mocks.commands.renamePlaylist).toHaveBeenCalledWith("p1", "白天");
    expect((await screen.findByRole("status")).textContent).toContain("已经有同名的歌单了");
    dialog = screen.getByRole("dialog", { name: "重命名歌单" });
    fireEvent.change(input, { target: { value: " 深夜跑 " } });
    fireEvent.click(within(dialog).getByRole("button", { name: "保存" }));
    expect(mocks.commands.renamePlaylist).toHaveBeenLastCalledWith("p1", " 深夜跑 ");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "重命名歌单" })).toBeNull());
});

test("删除歌单要确认：说明歌曲仍保留在「全部音乐」，删除后回到首页", async () => {
    render(<AndroidLayout />);
    fireEvent.click(screen.getByText("夜跑"));
    fireEvent.click(screen.getByRole("button", { name: "歌单更多操作" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "删除歌单" }));
    const dialog = screen.getByRole("alertdialog", { name: "删除歌单" });
    expect(dialog.textContent).toContain("确定删除「夜跑」？");
    expect(dialog.textContent).toContain("仍保留在「全部音乐」");
    fireEvent.click(within(dialog).getByRole("button", { name: "取消" }));
    expect(mocks.commands.deletePlaylist).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "歌单更多操作" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "删除歌单" }));
    fireEvent.click(
        within(screen.getByRole("alertdialog", { name: "删除歌单" })).getByRole("button", { name: "删除" }),
    );
    await waitFor(() => expect(mocks.commands.deletePlaylist).toHaveBeenCalledWith("p1"));
    expect((await screen.findByRole("status")).textContent).toContain("已删除歌单「夜跑」");
    expect(screen.getByRole("heading", { name: "歌单" })).toBeTruthy();
    expect(mocks.commands.removeTrack).not.toHaveBeenCalled();
});

test("编辑自建歌单：× 直接移出这个歌单，不动曲库文件", async () => {
    const { container } = render(<AndroidLayout />);
    fireEvent.click(screen.getByText("夜跑"));
    const list = page(container, "playlist");
    expect(within(list).queryByRole("button", { name: /从「夜跑」移除/ })).toBeNull();
    fireEvent.click(within(list).getByRole("button", { name: "编辑歌曲" }));
    // 状态写在标签文字和 data-editing 里：带 aria-pressed 时安卓 WebView 不再把按钮名字交给无障碍树
    expect(screen.getByRole("button", { name: "完成编辑歌曲" }).getAttribute("data-editing")).toBe("true");
    fireEvent.click(within(list).getByRole("button", { name: "从「夜跑」移除：春天" }));
    await waitFor(() => expect(mocks.commands.removePlaylistTrack).toHaveBeenCalledWith("p1", "a"));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(mocks.commands.removeTrack).not.toHaveBeenCalled();
    expect(mocks.commands.removeQueueTrack).not.toHaveBeenCalled();
    expect((await screen.findByRole("status")).textContent).toContain("已从「夜跑」移除");
    fireEvent.click(screen.getByRole("button", { name: "完成编辑歌曲" }));
    expect(within(list).queryByRole("button", { name: /从「夜跑」移除/ })).toBeNull();
});

test("编辑「全部音乐」：× 要确认，确认后从曲库移除", async () => {
    const { container } = render(<AndroidLayout />);
    const list = openAll(container);
    fireEvent.click(within(list).getByRole("button", { name: "编辑歌曲" }));
    const remove = within(list).getByRole("button", { name: "从「全部音乐」移除：Moon" });
    fireEvent.click(remove);
    const dialog = screen.getByRole("alertdialog", { name: "移除歌曲" });
    expect(dialog.textContent).toContain("确定从「全部音乐」移除《Moon》？");
    expect(dialog.textContent).toContain("曲库中的音乐文件也会被删除");
    fireEvent.click(within(dialog).getByRole("button", { name: "取消" }));
    expect(mocks.commands.removeTrack).not.toHaveBeenCalled();
    fireEvent.click(remove);
    fireEvent.click(
        within(screen.getByRole("alertdialog", { name: "移除歌曲" })).getByRole("button", { name: "确认移除" }),
    );
    await waitFor(() => expect(mocks.commands.removeTrack).toHaveBeenCalledWith("b"));
    expect(mocks.commands.removePlaylistTrack).not.toHaveBeenCalled();
    expect((await screen.findByRole("status")).textContent).toContain("已从曲库移除");
});

test("选歌页：已在歌单里的歌不能再选，添加后回到歌单", async () => {
    const { container } = render(<AndroidLayout />);
    fireEvent.click(screen.getByText("夜跑"));
    fireEvent.click(within(page(container, "playlist")).getByRole("button", { name: "添加歌曲到歌单" }));
    const add = page(container, "add");
    expect(within(add).getByRole("heading", { name: "添加到「夜跑」" })).toBeTruthy();
    expect((within(add).getByRole("button", { name: "已在歌单中：夜曲" }) as HTMLButtonElement).disabled).toBe(true);
    expect(within(add).queryByRole("button", { name: "已在歌单中：Moon" })).toBeNull();
    const confirm = () => within(add).getByRole("button", { name: /^添加( \(\d+\))?$/ });
    expect((confirm() as HTMLButtonElement).disabled).toBe(true);
    // 按点选的顺序加入
    fireEvent.click(within(add).getByText("Moon"));
    expect(confirm().textContent).toBe("添加 (1)");
    fireEvent.click(confirm());
    await waitFor(() => expect(mocks.commands.addPlaylistTracks).toHaveBeenCalledWith("p1", ["b"]));
    expect((await screen.findByRole("status")).textContent).toContain("已添加 1 首到「夜跑」");
    await waitFor(() => expect(container.querySelector('[data-page="add"]')).toBeNull());
    expect(within(page(container, "playlist")).getByRole("heading", { name: "夜跑" })).toBeTruthy();
});

test("选歌页的全选只选还没加入的歌，再点一次全部取消；没有可加的歌时不显示全选", () => {
    const view = render(<AndroidLayout />);
    fireEvent.click(screen.getByText("夜跑"));
    fireEvent.click(within(page(view.container, "playlist")).getByRole("button", { name: "添加歌曲到歌单" }));
    let add = page(view.container, "add");
    const all = within(add).getByRole("button", { name: "全选" });
    const confirm = () => within(add).getByRole("button", { name: /^添加( \(\d+\))?$/ });
    fireEvent.click(all);
    expect(all.getAttribute("aria-pressed")).toBe("true");
    expect(confirm().textContent).toBe("添加 (1)");
    fireEvent.click(all);
    expect(all.getAttribute("aria-pressed")).toBe("false");
    expect((confirm() as HTMLButtonElement).disabled).toBe(true);
    // 返回键先清空搜索
    fireEvent.change(search(add), { target: { value: "mo" } });
    expect(within(add).queryByText("夜曲")).toBeNull();
    act(() => mocks.back!());
    expect((search(add) as HTMLInputElement).value).toBe("");

    mocks.library = { ...library, playlists: [{ id: "p1", name: "夜跑", trackIds: ["a", "b", "c"] }] };
    view.rerender(<AndroidLayout />);
    add = page(view.container, "add");
    expect(within(add).queryByRole("button", { name: "全选" })).toBeNull();
    expect((within(add).getByRole("button", { name: "已在歌单中：Moon" }) as HTMLButtonElement).disabled).toBe(true);
});

test("播放列表弹层：点 × 直接移出播放列表，不确认，歌单和曲库不动", async () => {
    const { container } = render(<AndroidLayout />);
    fireEvent.click(screen.getByText("夜跑"));
    fireEvent.click(within(page(container, "playlist")).getByText("夜曲"));
    await waitFor(() => expect(mocks.commands.playPlaylist).toHaveBeenCalledWith("p1", "c"));
    fireEvent.click(await screen.findByRole("button", { name: "播放列表" }));
    const sheet = await screen.findByRole("dialog", { name: "播放列表" });
    expect(sheet.textContent).toContain("播放列表 (2)");
    fireEvent.click(within(sheet).getByRole("button", { name: "从播放列表移除：春天" }));
    await waitFor(() => expect(mocks.commands.removeQueueTrack).toHaveBeenCalledWith("a"));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(mocks.commands.removeTrack).not.toHaveBeenCalled();
    expect(mocks.commands.removePlaylistTrack).not.toHaveBeenCalled();
    expect((await screen.findByRole("status")).textContent).toContain("已从播放列表移除，歌单里仍保留");
});

test("导入页从「全部音乐」进入；导入后去听歌回到歌单页并定位到新导入的歌，不自动播放", async () => {
    mocks.commands.pickAndImport.mockResolvedValueOnce({
        added: ["a"],
        withLyrics: 1,
        lyricsAttached: 0,
        orphanLyrics: 0,
        duplicates: 0,
        sourcesKept: 0,
    });
    const { container } = render(<AndroidLayout />);
    const list = openAll(container);
    fireEvent.click(within(list).getByRole("button", { name: "导入音乐和歌词" }));
    expect(screen.getByRole("heading", { name: "导入音乐" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "选择歌曲" }).classList.contains("primary")).toBe(true);
    expect(screen.getByRole("button", { name: "导入整个文件夹" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "选择歌曲" }));
    expect(mocks.commands.pickAndImport).toHaveBeenCalledWith("files");
    expect(await screen.findByText("已导入 1 首，1 首有歌词")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "去听歌" }));
    expect(mocks.commands.playFrom).not.toHaveBeenCalled();
    expect(mocks.commands.playPlaylist).not.toHaveBeenCalled();
    expect(mocks.commands.playTrack).not.toHaveBeenCalled();
    await waitFor(() => expect(container.querySelector('[data-page="import"]')).toBeNull());
    expect(within(page(container, "playlist")).getByRole("heading", { name: "全部音乐" })).toBeTruthy();
    expect(page(container, "playlist").querySelector('.m-row[data-id="a"]')?.classList.contains("is-flash")).toBe(true);
});

test("取消选择时不显示导入结果；返回键在导入页回到歌单", async () => {
    const { container } = render(<AndroidLayout />);
    await waitFor(() => expect(mocks.back).not.toBeNull());
    const list = openAll(container);
    fireEvent.click(within(list).getByRole("button", { name: "导入音乐和歌词" }));
    fireEvent.click(screen.getByRole("button", { name: "导入整个文件夹" }));
    expect(mocks.commands.pickAndImport).toHaveBeenCalledWith("folder");
    await waitFor(() =>
        expect((screen.getByRole("button", { name: "导入整个文件夹" }) as HTMLButtonElement).disabled).toBe(false),
    );
    expect(screen.queryByRole("status")).toBeNull();
    act(() => mocks.back!());
    expect(screen.queryByRole("heading", { name: "导入音乐" })).toBeNull();
    expect(within(page(container, "playlist")).getByRole("heading", { name: "全部音乐" })).toBeTruthy();
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

test("切换播放模式显示中间偏下的深色轻提示", async () => {
    const { container } = render(<AndroidLayout />);
    await openPlayer(container);
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
    const { container } = render(<AndroidLayout />);
    await openPlayer(container);
    expect(screen.getByText("1.2X")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "升降调和倍速" }));
    expect(screen.getByRole("slider", { name: "升降调" })).toBeTruthy();
    expect(screen.getByRole("slider", { name: "倍速" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "恢复正常" })).toBeNull();
});

test("系统返回键：对话框 → 弹层 → 清空搜索 → 上一页，在首页时退到后台而不是关闭应用", async () => {
    const { container } = render(<AndroidLayout />);
    await waitFor(() => expect(mocks.back).not.toBeNull());
    fireEvent.click(screen.getByRole("button", { name: "新建歌单" }));
    act(() => mocks.back!());
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(mocks.commands.moveToBackground).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText("夜跑"));
    const list = page(container, "playlist");
    fireEvent.click(within(list).getByRole("button", { name: "歌单更多操作" }));
    act(() => mocks.back!());
    expect(screen.queryByRole("menu")).toBeNull();

    fireEvent.click(within(list).getByText("夜曲"));
    await screen.findByRole("button", { name: "收起" });
    fireEvent.click(screen.getByRole("button", { name: "播放列表" }));
    await screen.findByRole("dialog", { name: "播放列表" });
    act(() => mocks.back!());
    expect(screen.queryByRole("dialog", { name: "播放列表" })).toBeNull();
    expect(screen.getByRole("button", { name: "收起" })).toBeTruthy();
    act(() => mocks.back!());
    expect(screen.queryByRole("button", { name: "收起" })).toBeNull();

    fireEvent.change(search(list), { target: { value: "春" } });
    act(() => mocks.back!());
    expect((search(list) as HTMLInputElement).value).toBe("");
    expect(screen.getByRole("heading", { name: "夜跑" })).toBeTruthy();
    act(() => mocks.back!());
    expect(screen.getByRole("heading", { name: "歌单" })).toBeTruthy();
    expect(mocks.commands.moveToBackground).not.toHaveBeenCalled();
    act(() => mocks.back!());
    expect(mocks.commands.moveToBackground).toHaveBeenCalledOnce();
});

test("播放列表被清空时退出播放页，回到来源的歌单", async () => {
    const view = render(<AndroidLayout />);
    await openPlayer(view.container);
    mocks.state = { ...state, tracks: [], currentId: null, playing: false };
    view.rerender(<AndroidLayout />);
    await waitFor(() => expect(screen.queryByRole("button", { name: "收起" })).toBeNull());
    expect(within(page(view.container, "playlist")).getByRole("heading", { name: "全部音乐" })).toBeTruthy();
    expect(view.container.querySelector(".mini")).toBeNull();
});
