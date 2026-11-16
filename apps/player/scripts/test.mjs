// 播放器的快速测试：组件测试（Vitest）+ Tauri 外壳的 Rust 测试。
// 传了参数（测试文件、-t 用例）时只交给 Vitest，方便单独跑某个组件测试。
import { spawnSync } from "node:child_process";

const args = process.argv.slice(2);
function run(command, commandArgs) {
    const result = spawnSync(command, commandArgs, { stdio: "inherit", shell: process.platform === "win32" });
    if (result.status !== 0) process.exit(result.status ?? 1);
}

run("vitest", ["run", "--config", "vitest.config.ts", ...args]);
if (args.length === 0) run("cargo", ["test", "-p", "music-player-shell"]);
