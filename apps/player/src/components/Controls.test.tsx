import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";
import { ProgressBar } from "./ProgressBar";
import { TunePanel } from "./TunePanel";
import { ModeMenu } from "./ModeMenu";
import { Toast } from "./Toast";
import { formatTime } from "./formatTime";

afterEach(() => {
    cleanup();
    vi.useRealTimers();
});

function pointer(target: Element, type: string, x: number) {
    fireEvent(target, new MouseEvent(type, { bubbles: true, clientX: x, button: 0 }));
}

describe("播放进度", () => {
    test("拖动时预览，松开才提交；悬停显示对应时间", () => {
        const seek = vi.fn();
        const preview = vi.fn();
        render(<ProgressBar positionMs={10_000} durationMs={100_000} onSeek={seek} onPreviewChange={preview} />);
        const slider = screen.getByRole("slider", { name: "播放进度" });
        vi.spyOn(slider, "getBoundingClientRect").mockReturnValue({
            left: 100,
            right: 300,
            width: 200,
            top: 0,
            bottom: 10,
            height: 10,
            x: 100,
            y: 0,
            toJSON: () => ({}),
        });
        pointer(slider, "pointermove", 200);
        expect(screen.getByRole("tooltip").textContent).toBe("00:50");
        pointer(slider, "pointerdown", 140);
        pointer(slider, "pointermove", 250);
        expect(slider.getAttribute("aria-valuenow")).toBe("75");
        expect(preview).toHaveBeenLastCalledWith(75_000);
        expect(seek).not.toHaveBeenCalled();
        pointer(slider, "pointerup", 250);
        expect(seek).toHaveBeenCalledOnce();
        expect(seek).toHaveBeenCalledWith(75_000);
        expect(preview).toHaveBeenLastCalledWith(null);
    });
    test("方向键前后五秒并限制在歌曲范围内", () => {
        const seek = vi.fn();
        render(<ProgressBar positionMs={2_000} durationMs={30_000} onSeek={seek} />);
        const slider = screen.getByRole("slider", { name: "播放进度" });
        fireEvent.keyDown(slider, { key: "ArrowLeft" });
        fireEvent.keyDown(slider, { key: "ArrowRight" });
        expect(seek.mock.calls).toEqual([[0], [7_000]]);
    });
    test("未获取时长时不可跳转", () => {
        const seek = vi.fn();
        render(<ProgressBar positionMs={0} durationMs={null} onSeek={seek} />);
        const slider = screen.getByRole("slider", { name: "播放进度" });
        expect(slider.getAttribute("aria-disabled")).toBe("true");
        fireEvent.keyDown(slider, { key: "ArrowRight" });
        pointer(slider, "pointerdown", 10);
        pointer(slider, "pointerup", 20);
        expect(seek).not.toHaveBeenCalled();
    });
    test("取消拖动时清除预览且不跳转", () => {
        const seek = vi.fn();
        const preview = vi.fn();
        render(<ProgressBar positionMs={10_000} durationMs={100_000} onSeek={seek} onPreviewChange={preview} />);
        const slider = screen.getByRole("slider", { name: "播放进度" });
        vi.spyOn(slider, "getBoundingClientRect").mockReturnValue({
            left: 0,
            right: 100,
            width: 100,
            top: 0,
            bottom: 10,
            height: 10,
            x: 0,
            y: 0,
            toJSON: () => ({}),
        });
        pointer(slider, "pointerdown", 50);
        pointer(slider, "pointercancel", 50);
        expect(preview).toHaveBeenLastCalledWith(null);
        expect(slider.getAttribute("aria-valuenow")).toBe("10");
        expect(seek).not.toHaveBeenCalled();
    });
});

test("音效面板调节升降调（±5 档）和倍速，没有“恢复正常”", () => {
    const pitch = vi.fn();
    const rate = vi.fn();
    const { rerender } = render(<TunePanel pitch={-2} rate={1.2} onPitchChange={pitch} onRateChange={rate} />);
    expect(screen.getByText("降 2 个半音")).toBeTruthy();
    expect(screen.getByText("1.2倍播放")).toBeTruthy();
    fireEvent.change(screen.getByRole("slider", { name: "升降调" }), { target: { value: "3" } });
    fireEvent.change(screen.getByRole("slider", { name: "倍速" }), { target: { value: "0.8" } });
    expect(pitch).toHaveBeenCalledWith(3);
    expect(rate).toHaveBeenCalledWith(0.8);
    const pitchSlider = screen.getByRole("slider", { name: "升降调" });
    expect(pitchSlider.getAttribute("min")).toBe("-5");
    expect(pitchSlider.getAttribute("max")).toBe("5");
    expect(screen.queryByRole("button", { name: "恢复正常" })).toBeNull();
    rerender(<TunePanel pitch={0} rate={1} onPitchChange={pitch} onRateChange={rate} variant="mobile" />);
    expect(screen.queryByRole("button", { name: "恢复正常" })).toBeNull();
    expect(screen.getByText("升降调播放")).toBeTruthy();
});

test("播放模式菜单正确标识并返回所选模式", () => {
    const select = vi.fn();
    const close = vi.fn();
    render(<ModeMenu mode="repeat_one" onSelect={select} onClose={close} />);
    const current = screen.getByRole("menuitemradio", { name: "单曲循环" });
    expect(current.getAttribute("aria-checked")).toBe("true");
    current.focus();
    fireEvent.keyDown(current, { key: "ArrowDown" });
    expect(document.activeElement).toBe(screen.getByRole("menuitemradio", { name: "单曲播放" }));
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(close).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("menuitemradio", { name: "随机播放" }));
    expect(select).toHaveBeenCalledWith("shuffle");
});

test("提示自动消失并可重新提示", () => {
    vi.useFakeTimers();
    const dismiss = vi.fn();
    const { rerender } = render(<Toast message="列表循环" variant="mobile" onDismiss={dismiss} />);
    expect(screen.getByRole("status").textContent).toBe("列表循环");
    vi.advanceTimersByTime(1_800);
    expect(dismiss).toHaveBeenCalledOnce();
    rerender(<Toast message="随机播放" variant="mobile" onDismiss={dismiss} />);
    expect(screen.getByRole("status").textContent).toBe("随机播放");
});

test("按桌面和手机原型格式显示时间", () => {
    expect(formatTime(61_999)).toBe("01:01");
    expect(formatTime(61_999, "mobile")).toBe("1:01");
    expect(formatTime(null, "mobile")).toBe("--:--");
    expect(formatTime(0)).toBe("00:00");
});
