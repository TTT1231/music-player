export type TimeVariant = "desktop" | "mobile";

/** 毫秒转为“mm:ss”；未知时长在移动端显示占位。 */
export function formatTime(milliseconds: number | null | undefined, variant: TimeVariant = "desktop"): string {
    if (milliseconds == null || !Number.isFinite(milliseconds)) {
        return variant === "mobile" ? "--:--" : "00:00";
    }
    const seconds = Math.floor(Math.max(0, milliseconds) / 1_000);
    const minutes = Math.floor(seconds / 60);
    const minuteText = variant === "desktop" ? String(minutes).padStart(2, "0") : String(minutes);
    return `${minuteText}:${String(seconds % 60).padStart(2, "0")}`;
}
