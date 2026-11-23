import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { afterEach, expect, test, vi } from "vitest";
import { commands, listenLibrary, listenNotice, listenPlaybackEvents, listenPosition, listenState } from "./ipc";

afterEach(clearMocks);

test("typed commands send the documented names and parameters", async () => {
    const calls: Array<[string, unknown]> = [];
    mockIPC((command, args) => {
        calls.push([command, args]);
        return null;
    });
    await commands.appInfo();
    await commands.getState();
    await commands.getLibrary();
    await commands.importPaths(["song.mp3", "song.lrc"]);
    await commands.pickAndImport("folder");
    await commands.playTrack("track");
    await commands.playFrom(["first", "track"], "track");
    await commands.removeTrack("track");
    await commands.removeQueueTrack("track");
    await commands.play();
    await commands.pause();
    await commands.toggle();
    await commands.next();
    await commands.prev();
    await commands.seek(1500);
    await commands.setMode("repeat_one");
    await commands.setVolume(0.5);
    await commands.setRate(1.5);
    await commands.setPitch(-2);
    await commands.moveTrack(0, 2);
    await commands.createPlaylist(" 夜跑 ");
    await commands.renamePlaylist("p1", "深夜跑");
    await commands.addPlaylistTracks("p1", ["first", "track"]);
    await commands.removePlaylistTrack("p1", "first");
    await commands.deletePlaylist("p1");
    await commands.getLyrics("track");
    await commands.toggleDesktopLyrics();
    await commands.notifyState({
        playing: true,
        title: "t",
        artist: "a",
        durationMs: 180_000,
        positionMs: 5_000,
        rate: 1.2,
    });
    await commands.startPlaybackEvents();
    expect(calls).toEqual([
        ["app_info", {}],
        ["get_state", {}],
        ["get_library", {}],
        ["import_paths", { paths: ["song.mp3", "song.lrc"] }],
        ["pick_and_import", { kind: "folder" }],
        ["play_track", { id: "track" }],
        ["play_from", { ids: ["first", "track"], startId: "track" }],
        ["remove_track", { id: "track" }],
        ["remove_queue_track", { id: "track" }],
        ["play", {}],
        ["pause", {}],
        ["toggle", {}],
        ["next", {}],
        ["prev", {}],
        ["seek", { ms: 1500 }],
        ["set_mode", { mode: "repeat_one" }],
        ["set_volume", { volume: 0.5 }],
        ["set_rate", { rate: 1.5 }],
        ["set_pitch", { semitones: -2 }],
        ["move_track", { from: 0, to: 2 }],
        ["create_playlist", { name: " 夜跑 " }],
        ["rename_playlist", { id: "p1", name: "深夜跑" }],
        ["add_playlist_tracks", { id: "p1", tracks: ["first", "track"] }],
        ["remove_playlist_track", { id: "p1", track: "first" }],
        ["delete_playlist", { id: "p1" }],
        ["get_lyrics", { id: "track" }],
        ["toggle_desktop_lyrics", {}],
        [
            "plugin:playback|notify_state",
            { playing: true, title: "t", artist: "a", durationMs: 180_000, positionMs: 5_000, rate: 1.2 },
        ],
        ["plugin:playback|start_events", {}],
    ]);
});

test("event listeners expose the Rust payload and can be removed", async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const onState = vi.fn();
    const onPosition = vi.fn();
    const onNotice = vi.fn();
    const onLibrary = vi.fn();
    const offState = await listenState(onState);
    const offPosition = await listenPosition(onPosition);
    const offNotice = await listenNotice(onNotice);
    const offLibrary = await listenLibrary(onLibrary);
    await emit("state", { currentId: "song" });
    await emit("position", { ms: 1250 });
    await emit("notice", { kind: "unreadable", title: "song" });
    await emit("library", { tracks: [], playlists: [{ id: "p1", name: "夜跑", trackIds: ["song"] }] });
    expect(onState).toHaveBeenCalledWith({ currentId: "song" });
    expect(onPosition).toHaveBeenCalledWith({ ms: 1250 });
    expect(onNotice).toHaveBeenCalledWith({ kind: "unreadable", title: "song" });
    expect(onLibrary).toHaveBeenCalledWith({
        tracks: [],
        playlists: [{ id: "p1", name: "夜跑", trackIds: ["song"] }],
    });
    const offPlayback = await listenPlaybackEvents(event => {
        expect(event).toEqual({ kind: "pause", reason: "noisy" });
    });
    await emit("playback://event", { kind: "pause", reason: "noisy" });
    offState();
    offPosition();
    offNotice();
    offLibrary();
    offPlayback();
});

test("playlist commands and library-only imports use the IPC contract", async () => {
    const calls: Array<[string, unknown]> = [];
    mockIPC((command, args) => {
        calls.push([command, args]);
        return null;
    });
    await commands.importPaths(["song.mp3"], false);
    await commands.pickAndImport("files", false);
    await commands.createPlaylist("收藏");
    await commands.addPlaylistTracks("list-1", ["a", "b"]);
    await commands.removePlaylistTrack("list-1", "a");
    await commands.playPlaylist("list-1", "b");
    await commands.removeQueueTrack("a");
    expect(calls).toEqual([
        ["import_paths", { paths: ["song.mp3"], intoQueue: false }],
        ["pick_and_import", { kind: "files", intoQueue: false }],
        ["create_playlist", { name: "收藏" }],
        ["add_playlist_tracks", { id: "list-1", tracks: ["a", "b"] }],
        ["remove_playlist_track", { id: "list-1", track: "a" }],
        ["play_playlist", { id: "list-1", track: "b" }],
        ["remove_queue_track", { id: "a" }],
    ]);
});
