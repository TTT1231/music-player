import { useState, type CSSProperties } from "react";
import "./Cover.css";

const glazes = [
    ["#2440b8", "#eef0ea"],
    ["#8fae9a", "#1c2629"],
    ["#1c2629", "#c9d6c8"],
    ["#9b3b4a", "#f3e6e4"],
    ["#b08a3e", "#1c2629"],
    ["#d6e2ea", "#2440b8"],
    ["#4e6b5c", "#e3ece4"],
] as const;

function colorFor(id: string) {
    let hash = 2166136261;
    for (const char of id) {
        hash ^= char.codePointAt(0)!;
        hash = Math.imul(hash, 16777619);
    }
    return glazes[(hash >>> 0) % glazes.length];
}

export type CoverProps = {
    id: string;
    title: string;
    coverUrl?: string | null;
    /** 同一歌曲重新导入时更改，以重新请求封面。 */
    revision?: string | number;
    size?: number;
    className?: string;
};

export function Cover({ id, title, coverUrl, revision, size = 40, className = "" }: CoverProps) {
    return (
        <CoverContent
            key={JSON.stringify([id, coverUrl, revision])}
            id={id}
            title={title}
            coverUrl={coverUrl}
            revision={revision}
            size={size}
            className={className}
        />
    );
}

function CoverContent({ id, title, coverUrl, revision, size = 40, className = "" }: CoverProps) {
    const [failed, setFailed] = useState(false);
    const [background, foreground] = colorFor(id);
    const style = { "--s": `${size}px`, "--bg": background, "--fg": foreground } as CSSProperties;
    const classes = `cover ${className}`.trim();
    if (coverUrl && !failed) {
        const fragmentAt = coverUrl.indexOf("#");
        const url = fragmentAt < 0 ? coverUrl : coverUrl.slice(0, fragmentAt);
        const fragment = fragmentAt < 0 ? "" : coverUrl.slice(fragmentAt);
        const source =
            revision === undefined
                ? coverUrl
                : `${url}${url.includes("?") ? "&" : "?"}coverRevision=${encodeURIComponent(String(revision))}${fragment}`;
        return (
            <img
                className={classes}
                style={style}
                src={source}
                alt=""
                crossOrigin="anonymous"
                onError={() => setFailed(true)}
            />
        );
    }
    const glyph = title.match(/[\p{L}\p{N}]/u)?.[0].toUpperCase() ?? "♪";
    return (
        <div className={classes} style={style} aria-hidden="true">
            <span>{glyph}</span>
        </div>
    );
}
