import { readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
// Cargo workspace 的产物统一在仓库根的 target 下
const bundleDir = path.resolve(root, "target/release/bundle/nsis");

/** 安装包体积上限：15 MB */
export const MAX_INSTALLER_BYTES = 15 * 1024 * 1024;

/** CSP 源里的本地协议源（封面协议等），不算网络能力 */
const LOCAL_HTTP_SOURCES = /^http:\/\/(asset|cover|ipc)\.localhost$/;

/** CSP 源是否开放了外部网络；返回违规源，未违规返回 null */
export function networkSourceViolation(source: string): string | null {
    if (/^https?:\/\//.test(source) && !LOCAL_HTTP_SOURCES.test(source)) return source;
    return null;
}

/** 安卓 release 清单是否通过清单合并移除了 INTERNET 权限 */
export function androidReleaseManifestRemovesInternet(xml: string): boolean {
    const element = xml.match(/<uses-permission[^>]*android\.permission\.INTERNET[\s\S]*?\/>/);
    return element !== null && /tools:node=["']remove["']/.test(element[0]);
}

/** capability 权限是否疑似网络能力；返回违规权限，未违规返回 null */
export function capabilityViolation(permission: string): string | null {
    // 协议类权限（http、connect、fetch 等）都视为网络能力；core:window/event 等是本地 API
    if (/network|http|fetch|connect|websocket/i.test(permission)) return permission;
    return null;
}

function fail(message: string): never {
    console.error(`check:build 失败：${message}`);
    process.exit(1);
}

function readJson(relative: string): Record<string, unknown> {
    return JSON.parse(readFileSync(path.join(root, relative), "utf8")) as Record<string, unknown>;
}

/** NSIS 安装包存在且不超过体积上限 */
export function checkInstaller(): void {
    let installers: string[];
    try {
        installers = readdirSync(bundleDir).filter(name => name.endsWith(".exe"));
    } catch {
        fail(`找不到安装包目录 ${bundleDir}；先运行 pnpm --filter @music/player exec tauri build`);
    }
    if (installers.length === 0) fail("安装包目录里没有 .exe；先运行 pnpm --filter @music/player exec tauri build");
    for (const name of installers) {
        const size = statSync(path.join(bundleDir, name)).size;
        if (size > MAX_INSTALLER_BYTES) {
            fail(`${name} 体积 ${(size / 1024 / 1024).toFixed(2)} MB，超过 15 MB 上限`);
        }
        console.log(`✓ ${name} ${(size / 1024 / 1024).toFixed(2)} MB ≤ 15 MB`);
    }
}

/** 安卓 release 清单移除了 INTERNET 权限（开发模式仍需联网连 dev server，只移除 release 变体） */
export function checkAndroidReleaseManifest(): void {
    const mainPath = "apps/player/src-tauri/gen/android/app/src/main/AndroidManifest.xml";
    const releasePath = "apps/player/src-tauri/gen/android/app/src/release/AndroidManifest.xml";
    const main = readFileSync(path.join(root, mainPath), "utf8");
    if (!main.includes("android.permission.INTERNET")) {
        fail("主清单缺少 INTERNET 权限（开发模式需要连 dev server）");
    }
    let release: string;
    try {
        release = readFileSync(path.join(root, releasePath), "utf8");
    } catch {
        fail(`找不到 ${releasePath}；release 变体必须通过清单合并移除 INTERNET 权限`);
    }
    if (!androidReleaseManifestRemovesInternet(release)) {
        fail(`${releasePath} 没有用 tools:node="remove" 移除 INTERNET 权限`);
    }
    console.log("✓ 安卓 release 清单已移除 INTERNET 权限");
}

/** Tauri 配置与 capabilities 不开放网络能力 */
export function checkNoNetwork(): void {
    const config = readJson("apps/player/src-tauri/tauri.conf.json") as {
        app?: { security?: Record<string, unknown> };
    };
    for (const [name, csp] of Object.entries(config.app?.security ?? {})) {
        if (typeof csp !== "string") continue;
        for (const source of csp.split(/[\s;]+/)) {
            const violation = networkSourceViolation(source);
            if (violation) fail(`${name} CSP 包含外部网络源 ${violation}`);
        }
    }
    console.log("✓ Tauri CSP 没有外部网络源");

    const capabilitiesDir = path.join(root, "apps/player/src-tauri/capabilities");
    for (const name of readdirSync(capabilitiesDir)) {
        const capability = readJson(`apps/player/src-tauri/capabilities/${name}`) as {
            remote?: unknown;
            permissions?: string[];
        };
        if (capability.remote !== undefined) fail(`capability ${name} 开放了 remote 能力`);
        for (const permission of capability.permissions ?? []) {
            const violation = capabilityViolation(permission);
            if (violation) fail(`capability ${name} 的权限 ${violation} 疑似网络能力`);
        }
    }
    console.log("✓ capabilities 没有远程与网络权限");
}

// 只在直接运行时执行；被测试导入时不产生副作用
if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
    checkInstaller();
    checkNoNetwork();
    checkAndroidReleaseManifest();
    console.log("check:build 通过");
}
