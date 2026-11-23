import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/**
 * 沙箱（Codex / DSH 等）在仓库里跑过之后，目录会带上 Low 完整性标签。
 * 从这里编译出的 exe 继承 Low，运行时 WebView2 写不了自己的配置目录，
 * 启动即报 0x800700AA（请求的资源在使用中）或 0x8000FFFF（灾难性故障）。
 */
export const LOW_INTEGRITY_LABEL = /low mandatory level|低强制级别/i;

/** icacls 输出里是否带 Low 完整性标签；输出会被本地化成中文，两种都认 */
export function hasLowIntegrityLabel(icaclsOutput: string): boolean {
    return LOW_INTEGRITY_LABEL.test(icaclsOutput);
}

export type IntegrityResult = "skipped" | "ok" | "repaired" | "failed";

/** 失败返回 null：要么没有 icacls，要么当前环境不允许改 ACL，都不该挡住 dev */
function icacls(args: string[]): string | null {
    try {
        return execFileSync("icacls", args, {
            encoding: "utf8",
            stdio: ["ignore", "pipe", "ignore"],
            windowsHide: true,
        });
    } catch {
        return null;
    }
}

function labelIsLow(target: string): boolean {
    const output = icacls([target]);
    return output !== null && hasLowIntegrityLabel(output);
}

/** 目录用 (OI)(CI)M 覆盖整棵子树；文件只改自身 */
function resetLabel(target: string, directory: boolean): boolean {
    return icacls([target, "/setintegritylevel", directory ? "(OI)(CI)M" : "M"]) !== null;
}

/**
 * 检查并修复仓库与构建产物的完整性级别：Low 改回 Medium。
 * 只做这一件事，正常状态不输出、不改动。
 */
export function ensureIntegrity(repoRoot: string = root): IntegrityResult {
    if (process.platform !== "win32") return "skipped";

    const executable = path.join(repoRoot, "target", "debug", "music-player-shell.exe");
    const rootIsLow = labelIsLow(repoRoot);
    const executableIsLow = existsSync(executable) && labelIsLow(executable);
    if (!rootIsLow && !executableIsLow) return "ok";

    let repaired = true;
    if (rootIsLow) repaired = resetLabel(repoRoot, true) && repaired;
    // 目录标签会向下继承，但已经存在的构建产物要单独确认
    if (executableIsLow && labelIsLow(executable)) repaired = resetLabel(executable, false) && repaired;
    return repaired ? "repaired" : "failed";
}

// 只在直接运行时执行；被 stop 和测试导入时不产生副作用
if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
    const result = ensureIntegrity();
    if (result === "repaired") console.log("🔧 已把沙箱遗留的 Low 完整性标签重置为 Medium");
    if (result === "failed") console.warn("⚠ 无法重置 Low 完整性标签，桌面版可能仍然创建不了 WebView2");
}
