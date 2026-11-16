import { useEffect, type RefObject } from "react";

/** 每帧向目标靠近剩余距离的比例；60fps 下约 0.3 秒到位。 */
const EASE = 0.2;

/**
 * 滚轮平滑滚动：滚轮不再一下跳到位，而是缓动到目标位置，连续滚动时目标累加。
 * 其他途径改动了滚动位置（拖动滚动条、拖动排序时的自动滚动、脚本定位）时立即让出。
 * Ctrl+滚轮（缩放）、横向滚动和系统“减少动态效果”保持浏览器默认行为。
 */
export function useSmoothWheel(ref: RefObject<HTMLElement | null>): void {
    useEffect(() => {
        const element = ref.current;
        if (!element) return;
        let target = 0;
        let last = 0;
        let frame: number | null = null;
        const max = () => Math.max(0, element.scrollHeight - element.clientHeight);
        const tick = () => {
            frame = null;
            if (Math.abs(element.scrollTop - last) > 1) return;
            const diff = target - element.scrollTop;
            // 至少走 1px：屏幕缩放下滚动位置按设备像素取整，步子太小会停在原地
            const step = Math.sign(diff) * Math.max(1, Math.abs(diff) * EASE);
            element.scrollTop = Math.abs(diff) <= 1 ? target : element.scrollTop + step;
            last = element.scrollTop;
            if (Math.abs(target - last) > 0.5) frame = requestAnimationFrame(tick);
            else element.scrollTop = last = target;
        };
        const wheel = (event: WheelEvent) => {
            if (event.ctrlKey || Math.abs(event.deltaX) > Math.abs(event.deltaY)) return;
            if (
                typeof window.matchMedia === "function" &&
                window.matchMedia("(prefers-reduced-motion: reduce)").matches
            )
                return;
            event.preventDefault();
            const unit = event.deltaMode === 1 ? 40 : event.deltaMode === 2 ? element.clientHeight : 1;
            if (frame === null) target = element.scrollTop;
            target = Math.min(max(), Math.max(0, target + event.deltaY * unit));
            last = element.scrollTop;
            if (frame === null) frame = requestAnimationFrame(tick);
        };
        element.addEventListener("wheel", wheel, { passive: false });
        return () => {
            element.removeEventListener("wheel", wheel);
            if (frame !== null) cancelAnimationFrame(frame);
        };
    }, [ref]);
}
