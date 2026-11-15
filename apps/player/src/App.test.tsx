import { cleanup, render } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, expect, test } from "vitest";
import App from "./App";

afterEach(() => {
    cleanup();
    clearMocks();
});

test("renders the initial empty player", () => {
    const { container } = render(<App />);
    expect(container.querySelector("main")).not.toBeNull();
    expect(container.textContent).toBe("");
});

test("allows component tests to mock Tauri commands", async () => {
    mockIPC(command => (command === "app_info" ? { platform: "desktop" } : null));
    await expect(invoke("app_info")).resolves.toEqual({ platform: "desktop" });
});
