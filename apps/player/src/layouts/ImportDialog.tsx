import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type ImportResult } from "../ipc";
import { importSummary } from "../components/importSummary";
import "./ImportDialog.css";

export type ImportDialogProps = {
    onClose: () => void;
    onImported: (ids: string[]) => void;
    onGoListen: (firstId: string | null) => void;
    droppedPaths?: string[] | null;
    dragging?: boolean;
};

function MusicIcon() {
    return (
        <svg className="import-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M9 18V6l11-2v12" />
            <circle cx="6.5" cy="18" r="2.5" />
            <circle cx="17.5" cy="16" r="2.5" />
        </svg>
    );
}
function FolderIcon() {
    return (
        <svg className="import-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
        </svg>
    );
}

export function ImportDialog({ onClose, onImported, onGoListen, droppedPaths, dragging = false }: ImportDialogProps) {
    const [result, setResult] = useState<ImportResult | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [busy, setBusy] = useState(false);
    const importing = useRef(false);
    const mounted = useRef(false);
    const lastDrop = useRef<string[] | null | undefined>(null);
    const importedCallback = useRef(onImported);
    useEffect(() => {
        importedCallback.current = onImported;
    }, [onImported]);
    useEffect(() => {
        mounted.current = true;
        return () => {
            mounted.current = false;
        };
    }, []);
    useEffect(() => {
        const handleKey = (event: KeyboardEvent) => {
            if (event.key === "Escape") {
                event.stopPropagation();
                onClose();
            }
        };
        document.addEventListener("keydown", handleKey);
        return () => document.removeEventListener("keydown", handleKey);
    }, [onClose]);

    const run = useCallback(async (operation: () => Promise<ImportResult | null>) => {
        if (importing.current) return;
        importing.current = true;
        if (mounted.current) {
            setBusy(true);
            setError(null);
        }
        try {
            const imported = await operation();
            if (imported) {
                importedCallback.current(imported.added);
                if (mounted.current) setResult(imported);
            }
        } catch (cause) {
            if (mounted.current) {
                setResult(null);
                setError(String(cause));
            }
        } finally {
            importing.current = false;
            if (mounted.current) setBusy(false);
        }
    }, []);

    useEffect(() => {
        if (droppedPaths && droppedPaths.length && lastDrop.current !== droppedPaths) {
            lastDrop.current = droppedPaths;
            void run(() => commands.importPaths(droppedPaths));
        }
    }, [droppedPaths, run]);

    const success = Boolean(result && (result.added.length || result.lyricsAttached));
    return (
        <div
            className="import-backdrop"
            data-testid="import-backdrop"
            onClick={event => {
                if (event.target === event.currentTarget) onClose();
            }}
        >
            <div className="import-dialog" role="dialog" aria-modal="true" aria-labelledby="import-title">
                <button className="import-close" aria-label="关闭" onClick={onClose}>
                    <svg className="import-icon" viewBox="0 0 24 24" aria-hidden="true">
                        <path d="M6 6l12 12M18 6 6 18" />
                    </svg>
                </button>
                <h2 id="import-title">导入音乐</h2>
                <div className={`import-drop${dragging ? " over" : ""}`}>
                    <span className="import-drop-icon">
                        <MusicIcon />
                    </span>
                    <strong>把歌曲拖到这里</strong>
                </div>
                <div className="import-actions">
                    <button
                        className={`import-button${success ? "" : " primary"}`}
                        disabled={busy}
                        onClick={() => void run(() => commands.pickAndImport("files"))}
                    >
                        <MusicIcon />
                        选择歌曲
                    </button>
                    <button
                        className="import-button"
                        disabled={busy}
                        onClick={() => void run(() => commands.pickAndImport("folder"))}
                    >
                        <FolderIcon />
                        导入整个文件夹
                    </button>
                </div>
                {(result || error) && (
                    <div className="import-result" role="status">
                        <p>{error ?? (result && importSummary(result))}</p>
                        {success && (
                            <button
                                className="import-button primary"
                                onClick={() => {
                                    onGoListen(result?.added[0] ?? null);
                                    onClose();
                                }}
                            >
                                去听歌
                            </button>
                        )}
                    </div>
                )}
            </div>
        </div>
    );
}
