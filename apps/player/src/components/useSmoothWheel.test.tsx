import { cleanup, fireEvent, render } from "@testing-library/react";
import { useRef } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { useSmoothWheel } from "./useSmoothWheel";

function List() {
    const ref = useRef<HTMLDivElement>(null);
    useSmoothWheel(ref);
    return <div ref={ref} data-testid="list" />;
}

function setup() {
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => frames.push(callback));
    vi.stubGlobal("cancelAnimationFrame", vi.fn());
    const view = render(<List />);
    const list = view.getByTestId("list");
    Object.defineProperty(list, "scrollHeight", { value: 2000 });
    Object.defineProperty(list, "clientHeight", { value: 500 });
    const step = () => frames.splice(0).forEach(callback => callback(0));
    return { list, step };
}

afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
});

test("wheel scrolling eases towards the target over several frames", () => {
    const { list, step } = setup();
    const event = fireEvent.wheel(list, { deltaY: 300 });
    expect(event).toBe(false); // 默认的跳跃式滚动被接管
    step();
    const first = list.scrollTop;
    expect(first).toBeGreaterThan(0);
    expect(first).toBeLessThan(300);
    for (let i = 0; i < 60; i++) step();
    expect(list.scrollTop).toBe(300);
});

test("successive notches add up and stop at the end of the list", () => {
    const { list, step } = setup();
    fireEvent.wheel(list, { deltaY: 1000 });
    fireEvent.wheel(list, { deltaY: 1000 });
    for (let i = 0; i < 80; i++) step();
    expect(list.scrollTop).toBe(1500);
});

test("yields when something else moves the list", () => {
    const { list, step } = setup();
    fireEvent.wheel(list, { deltaY: 400 });
    step();
    list.scrollTop = 900; // 拖动滚动条或脚本定位
    for (let i = 0; i < 30; i++) step();
    expect(list.scrollTop).toBe(900);
});

test("zoom gestures keep the default behaviour", () => {
    const { list } = setup();
    expect(fireEvent.wheel(list, { deltaY: 100, ctrlKey: true })).toBe(true);
});
