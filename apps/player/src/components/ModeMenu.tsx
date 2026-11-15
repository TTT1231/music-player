import type { PlayMode } from "../ipc";
import "./controls.css";

export const MODES: ReadonlyArray<{ id: PlayMode; label: string; path: string }> = [
    { id: "order", label: "顺序播放", path: "M4 8h15l-3-3M20 16H5l3 3" },
    { id: "loop", label: "列表循环", path: "M4 11V9a3 3 0 0 1 3-3h13l-3-3M20 13v2a3 3 0 0 1-3 3H4l3 3" },
    {
        id: "shuffle",
        label: "随机播放",
        path: "M3 7h3.5c4 0 6.5 10 11 10H21M3 17h3.5c1.6 0 3-1.6 4.2-3.6M14 8.6c1-1 2.2-1.6 3.5-1.6H21M18 4l3 3-3 3M18 14l3 3-3 3",
    },
    {
        id: "repeat_one",
        label: "单曲循环",
        path: "M4 11V9a3 3 0 0 1 3-3h13l-3-3M20 13v2a3 3 0 0 1-3 3H4l3 3M11 10.6 12.6 9.6v5",
    },
    { id: "single", label: "单曲播放", path: "M4 12h15l-3-3" },
];

export interface ModeMenuProps {
    mode: PlayMode;
    onSelect: (mode: PlayMode) => void;
    onClose?: () => void;
}

export function ModeMenu({ mode, onSelect, onClose }: ModeMenuProps) {
    return (
        <div
            className="player-mode-menu"
            role="menu"
            aria-label="播放模式"
            onKeyDown={event => {
                if (event.key === "Escape") {
                    event.preventDefault();
                    onClose?.();
                    return;
                }
                if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
                const items = Array.from(
                    event.currentTarget.querySelectorAll<HTMLButtonElement>('button[role="menuitemradio"]'),
                );
                const current = items.indexOf(document.activeElement as HTMLButtonElement);
                const next =
                    event.key === "ArrowDown"
                        ? (current + 1) % items.length
                        : (current - 1 + items.length) % items.length;
                event.preventDefault();
                items[next]?.focus();
            }}
        >
            {MODES.map(item => (
                <button
                    type="button"
                    key={item.id}
                    role="menuitemradio"
                    aria-checked={mode === item.id}
                    onClick={() => onSelect(item.id)}
                >
                    <svg viewBox="0 0 24 24" aria-hidden="true">
                        <path d={item.path} />
                    </svg>
                    <span>{item.label}</span>
                </button>
            ))}
        </div>
    );
}
