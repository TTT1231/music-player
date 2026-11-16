import type { CSSProperties } from "react";
import "./controls.css";

export interface TunePanelProps {
    pitch: number;
    rate: number;
    onPitchChange: (semitones: number) => void;
    onRateChange: (rate: number) => void;
    variant?: "desktop" | "mobile";
}

/** 升降调每侧档数，与 Rust 的 PITCH_RANGE、倍速每侧 5 档（0.5–1.5）一致 */
const PITCH_STEPS = 5;

const rangeStyle = (value: number, min: number, max: number) => {
    const percent = Math.max(0, Math.min(100, ((value - min) / (max - min)) * 100));
    return { "--a": `${Math.min(50, percent)}%`, "--b": `${Math.max(50, percent)}%` } as CSSProperties;
};

export function TunePanel({ pitch, rate, onPitchChange, onRateChange, variant = "desktop" }: TunePanelProps) {
    return (
        <div className={`player-tune player-tune--${variant}`}>
            <label className="player-tune__row">
                <span className="player-tune__title">
                    {pitch === 0 ? "升降调播放" : `${pitch > 0 ? "升" : "降"} ${Math.abs(pitch)} 个半音`}
                </span>
                <input
                    aria-label="升降调"
                    className="player-tune__range"
                    type="range"
                    min={-PITCH_STEPS}
                    max={PITCH_STEPS}
                    step="1"
                    value={pitch}
                    style={rangeStyle(pitch, -PITCH_STEPS, PITCH_STEPS)}
                    onChange={event => onPitchChange(Number(event.currentTarget.value))}
                />
                <span className="player-tune__ends">
                    <span>降调</span>
                    <span>正常</span>
                    <span>升调</span>
                </span>
            </label>
            <label className="player-tune__row">
                <span className="player-tune__title">
                    {Math.abs(rate - 1) < 0.01 ? "倍速播放" : `${rate.toFixed(1)}倍播放`}
                </span>
                <input
                    aria-label="倍速"
                    className="player-tune__range"
                    type="range"
                    min="0.5"
                    max="1.5"
                    step="0.1"
                    value={rate}
                    style={rangeStyle(rate, 0.5, 1.5)}
                    onChange={event => onRateChange(Number(event.currentTarget.value))}
                />
                <span className="player-tune__ends">
                    <span>减慢</span>
                    <span>正常</span>
                    <span>加快</span>
                </span>
            </label>
        </div>
    );
}
