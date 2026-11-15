import { useRef, useState, type PointerEvent } from "react";
import { formatTime, type TimeVariant } from "./formatTime";
import "./controls.css";

export interface ProgressBarProps {
    positionMs: number;
    durationMs: number | null;
    onSeek: (milliseconds: number) => void;
    onPreviewChange?: (milliseconds: number | null) => void;
    variant?: TimeVariant;
}

export function ProgressBar({
    positionMs,
    durationMs,
    onSeek,
    onPreviewChange,
    variant = "desktop",
}: ProgressBarProps) {
    const duration = durationMs && Number.isFinite(durationMs) && durationMs > 0 ? durationMs : 0;
    const [preview, setPreview] = useState<number | null>(null);
    const [hover, setHover] = useState<number | null>(null);
    const dragging = useRef(false);
    const clamp = (value: number) => Math.max(0, Math.min(duration, Number.isFinite(value) ? value : 0));
    const position = clamp(preview ?? positionMs);
    const percent = duration ? (position / duration) * 100 : 0;
    const valueAt = (element: HTMLElement, x: number) => {
        const rect = element.getBoundingClientRect();
        return rect.width > 0 ? Math.round(clamp(((x - rect.left) / rect.width) * duration)) : 0;
    };
    const updatePreview = (event: PointerEvent<HTMLDivElement>) => {
        const next = valueAt(event.currentTarget, event.clientX);
        setPreview(next);
        onPreviewChange?.(next);
    };
    const finish = () => {
        dragging.current = false;
        setPreview(null);
        onPreviewChange?.(null);
    };
    return (
        <div
            className={`player-progress player-progress--${variant}${preview !== null ? " is-dragging" : ""}`}
            role="slider"
            tabIndex={0}
            aria-label="播放进度"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(percent)}
            aria-valuetext={`${formatTime(position, variant)}/${formatTime(durationMs, variant)}`}
            aria-disabled={!duration}
            onPointerDown={event => {
                if (!duration || event.button !== 0) return;
                dragging.current = true;
                event.currentTarget.setPointerCapture?.(event.pointerId);
                updatePreview(event);
            }}
            onPointerMove={event => {
                if (!duration) return;
                setHover(valueAt(event.currentTarget, event.clientX));
                if (dragging.current) updatePreview(event);
            }}
            onPointerLeave={() => {
                if (!dragging.current) setHover(null);
            }}
            onPointerUp={event => {
                if (!dragging.current) return;
                const next = valueAt(event.currentTarget, event.clientX);
                finish();
                onSeek(next);
            }}
            onPointerCancel={() => {
                if (dragging.current) finish();
                setHover(null);
            }}
            onKeyDown={event => {
                const offset = event.key === "ArrowRight" ? 5_000 : event.key === "ArrowLeft" ? -5_000 : 0;
                if (!offset || !duration) return;
                event.preventDefault();
                onSeek(clamp(positionMs + offset));
            }}
        >
            <div className="player-progress__track">
                <span className="player-progress__played" style={{ width: `${percent}%` }} />
                <span className="player-progress__knob" style={{ left: `${percent}%` }} />
            </div>
            {hover !== null && duration > 0 && (
                <span
                    className="player-progress__tip"
                    role="tooltip"
                    style={{ left: `clamp(28px, ${(hover / duration) * 100}%, calc(100% - 28px))` }}
                >
                    {formatTime(hover, variant)}
                </span>
            )}
        </div>
    );
}
