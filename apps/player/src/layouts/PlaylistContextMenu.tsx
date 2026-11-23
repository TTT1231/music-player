import { useEffect, useLayoutEffect, useRef } from "react";
import { createPortal } from "react-dom";
import type { SavedPlaylist } from "../ipc";

export type PlaylistMenuPosition = {
    playlist: SavedPlaylist;
    trigger: HTMLButtonElement;
    x: number;
    y: number;
};

type Props = {
    menu: PlaylistMenuPosition;
    onDismiss: () => void;
    onDelete: () => void;
};

export function PlaylistContextMenu({ menu, onDismiss, onDelete }: Props) {
    const surface = useRef<HTMLDivElement>(null);
    const item = useRef<HTMLButtonElement>(null);

    useLayoutEffect(() => {
        const element = surface.current!;
        const { width, height } = element.getBoundingClientRect();
        element.style.left = `${Math.max(4, Math.min(menu.x, window.innerWidth - width - 4))}px`;
        element.style.top = `${Math.max(4, Math.min(menu.y, window.innerHeight - height - 4))}px`;
        item.current?.focus({ preventScroll: true });
    }, [menu]);

    useEffect(() => {
        const outside = (event: PointerEvent) => {
            if (event.target instanceof Node && !surface.current?.contains(event.target)) onDismiss();
        };
        document.addEventListener("pointerdown", outside, true);
        window.addEventListener("resize", onDismiss);
        return () => {
            document.removeEventListener("pointerdown", outside, true);
            window.removeEventListener("resize", onDismiss);
        };
    }, [onDismiss]);

    const label = `删除歌单「${menu.playlist.name}」`;
    return createPortal(
        <div ref={surface} className="playlist-context-menu" role="menu" aria-label={`「${menu.playlist.name}」的操作`}>
            <button
                ref={item}
                type="button"
                role="menuitem"
                tabIndex={-1}
                title={label}
                onBlur={onDismiss}
                onClick={onDelete}
                onKeyDown={event => {
                    if (event.key === "Escape") {
                        event.preventDefault();
                        event.stopPropagation();
                        onDismiss();
                        menu.trigger.focus({ preventScroll: true });
                    }
                }}
            >
                {label}
            </button>
        </div>,
        document.body,
    );
}
