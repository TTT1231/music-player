import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { ensureIntegrity } from "./ensure-integrity.js";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/** Cargo 包名决定主程序文件名；崩溃或强退后它可能还占着 WebView2 的数据目录 */
export const APP_IMAGE = "music-player-shell.exe";

/** 残留的 WebView2 子进程命令行里带应用标识，只清理本应用的 */
export const APP_IDENTIFIER = "local.music.player";

export const DEFAULT_DEV_PORT = 1420;

interface TauriConfig {
    build?: { beforeDevCommand?: string };
}

interface CommandResult {
    ok: boolean;
    stdout: string;
    status: number | null;
}

/** 同步执行命令；非 0 退出不算异常，交给调用方判断 */
function run(command: string, args: string[]): CommandResult {
    try {
        // 丢弃 stderr：taskkill 的本地化提示按控制台代码页输出，重定向后会变成乱码
        const stdout = execFileSync(command, args, {
            encoding: "utf8",
            stdio: ["ignore", "pipe", "ignore"],
            windowsHide: true,
        });
        return { ok: true, stdout, status: 0 };
    } catch (error) {
        const failure = error as { status?: number | null; stdout?: string };
        return { ok: false, stdout: failure.stdout ?? "", status: failure.status ?? null };
    }
}

/** 从 tauri.conf.json 的 beforeDevCommand 读端口，脚本不另写一份配置 */
export function devPortFromConfig(config: TauriConfig): number {
    const match = /--port\s+(\d+)/.exec(config.build?.beforeDevCommand ?? "");
    return match ? Number(match[1]) : DEFAULT_DEV_PORT;
}

/** 解析 Windows `netstat -ano` 输出里监听指定端口的 PID */
export function listeningPidsFromNetstat(output: string, port: number): number[] {
    const pids = new Set<number>();
    for (const line of output.split(/\r?\n/)) {
        const match = /^\s*TCP\s+\S+:(\d+)\s+\S+\s+LISTENING\s+(\d+)\s*$/i.exec(line);
        if (match && Number(match[1]) === port) pids.add(Number(match[2]));
    }
    return [...pids];
}

/** 解析 `lsof -ti tcp:<port> -sTCP:LISTEN` 的 PID 列表 */
export function listeningPidsFromLsof(output: string): number[] {
    const pids = new Set<number>();
    for (const token of output.split(/\s+/)) {
        if (/^\d+$/.test(token)) pids.add(Number(token));
    }
    return [...pids];
}

function killImage(image: string): boolean {
    const result = run("taskkill", ["/F", "/T", "/IM", image]);
    if (result.ok) return true;
    // 128 表示没有匹配的进程，属于正常情况；其余退出码才是失败
    if (result.status !== 128) console.warn(`⚠ 结束 ${image} 失败（taskkill 退出码 ${result.status ?? "未知"}）`);
    return false;
}

function killPid(pid: number): boolean {
    if (process.platform === "win32") return run("taskkill", ["/F", "/T", "/PID", String(pid)]).ok;
    try {
        process.kill(pid, "SIGKILL");
        return true;
    } catch {
        return false;
    }
}

function listeningPids(port: number): number[] {
    if (process.platform === "win32") {
        const result = run("netstat", ["-ano", "-p", "TCP"]);
        return result.ok ? listeningPidsFromNetstat(result.stdout, port) : [];
    }
    const result = run("lsof", ["-ti", `tcp:${port}`, "-sTCP:LISTEN"]);
    return result.ok ? listeningPidsFromLsof(result.stdout) : [];
}

/** 主程序被强退后 WebView2 仍可能存活并锁住数据目录，按命令行里的应用标识清理 */
function orphanWebView2Pids(): number[] {
    const query =
        `Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | ` +
        `Where-Object { $_.CommandLine -like '*${APP_IDENTIFIER}*' } | ` +
        "Select-Object -ExpandProperty ProcessId";
    const result = run("powershell", ["-NoProfile", "-NonInteractive", "-Command", query]);
    if (!result.ok) return [];
    return result.stdout
        .split(/\r?\n/)
        .map(line => line.trim())
        .filter(line => /^\d+$/.test(line))
        .map(Number);
}

function readDevPort(): number {
    const file = path.join(root, "apps/player/src-tauri/tauri.conf.json");
    return devPortFromConfig(JSON.parse(readFileSync(file, "utf8")) as TauriConfig);
}

function stopDesktop(): void {
    const port = readDevPort();
    console.log(`🚀 停止桌面版 dev 进程（端口 ${port}）`);

    let stopped = 0;
    if (process.platform === "win32" && killImage(APP_IMAGE)) {
        console.log(`✅ 结束 ${APP_IMAGE}`);
        stopped += 1;
    }

    for (const pid of listeningPids(port)) {
        if (killPid(pid)) {
            console.log(`✅ 结束占用端口 ${port} 的进程 ${pid}`);
            stopped += 1;
        } else {
            console.warn(`⚠ 结束进程 ${pid} 失败`);
        }
    }

    if (process.platform === "win32") {
        for (const pid of orphanWebView2Pids()) {
            if (killPid(pid)) {
                console.log(`✅ 结束残留的 WebView2 进程 ${pid}`);
                stopped += 1;
            }
        }
    }

    // 沙箱把仓库打成 Low 完整性时，0x800700AA 跟残留进程无关，stop 顺手修掉
    const integrity = ensureIntegrity();
    if (integrity === "repaired") {
        console.log("✅ 已把沙箱遗留的 Low 完整性标签重置为 Medium");
    } else if (integrity === "failed") {
        console.warn("⚠ 无法重置 Low 完整性标签，桌面版可能仍然报“请求的资源在使用中”");
    }

    if (stopped === 0) {
        console.log("ℹ 没有找到运行中的桌面版 dev 进程");
    }

    const remaining = listeningPids(port);
    if (remaining.length > 0) {
        console.error(`❌ 端口 ${port} 仍被进程 ${remaining.join("、")} 占用，请手动结束后再运行 pnpm desktop`);
        process.exit(1);
    }
    console.log(`✨ 端口 ${port} 已释放，可以重新运行 pnpm desktop`);
}

// 只在直接运行时执行；被测试导入时不产生副作用
if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
    stopDesktop();
}
