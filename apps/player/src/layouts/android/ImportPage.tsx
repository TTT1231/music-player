import { useState } from "react";
import { importSummary } from "../../components/importSummary";
import { commands, type ImportResult } from "../../ipc";
import { BackButton, Icon, Page, icons, type PageProps } from "./shared";

export type ImportPageProps = PageProps & {
    onBack: () => void;
    /** 「去听歌」：带着这次新导入的歌曲回到「全部音乐」。 */
    onGoListen: (added: string[]) => void;
    showToast: (message: string) => void;
};

/** 导入页：系统文件选择器选歌曲或整个文件夹，导入的歌进入「全部音乐」。 */
export function ImportPage({ level, active, leaving, onBack, onGoListen, showToast }: ImportPageProps) {
    const [result, setResult] = useState<ImportResult | null>(null);
    const [busy, setBusy] = useState(false);
    const imported = Boolean(result && (result.added.length || result.lyricsAttached));

    // 选择与导入都在外壳完成：系统文件选择器提供文件名，歌名和歌词配对才正确；取消时返回 null
    const pick = async (kind: "files" | "folder") => {
        setBusy(true);
        try {
            const next = await commands.pickAndImport(kind);
            if (next) setResult(next);
        } catch (reason) {
            showToast(String(reason));
        } finally {
            setBusy(false);
        }
    };

    return (
        <Page kind="import" level={level} active={active} leaving={leaving} label="导入音乐">
            <div className="page-body">
                <div className="imp-top">
                    <BackButton onClick={onBack} />
                </div>
                <div className="imp">
                    <div className="imp-hero" aria-hidden="true">
                        <Icon path={icons.note} />
                    </div>
                    <h2>导入音乐</h2>
                    <button
                        type="button"
                        className={`imp-btn${imported ? "" : " primary"}`}
                        disabled={busy}
                        onClick={() => void pick("files")}
                    >
                        <Icon path={icons.note} />
                        选择歌曲
                    </button>
                    <button type="button" className="imp-btn" disabled={busy} onClick={() => void pick("folder")}>
                        <Icon path={icons.folder} />
                        导入整个文件夹
                    </button>
                    {result && (
                        <div className="imp-done" role="status">
                            <p>{importSummary(result)}</p>
                            {imported && (
                                <button
                                    type="button"
                                    className="imp-btn primary"
                                    onClick={() => {
                                        const added = result.added;
                                        setResult(null);
                                        onGoListen(added);
                                    }}
                                >
                                    去听歌
                                </button>
                            )}
                        </div>
                    )}
                </div>
            </div>
        </Page>
    );
}
