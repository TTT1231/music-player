import { cleanup, render } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, expect, test } from "vitest";
import App from "./App";

afterEach(() => {
    cleanup();
    clearMocks();
});

test("renders a status while the player starts", () => {
    const { container } = render(<App />);
    expect(container.querySelector("main")).not.toBeNull();
    expect(container.textContent).toBe("正在打开播放器…");
});

test("allows component tests to mock Tauri commands", async () => {
    mockIPC(command => (command === "app_info" ? { platform: "desktop" } : null));
    await expect(invoke("app_info")).resolves.toEqual({ platform: "desktop" });
});
