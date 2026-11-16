# 测试与类型检查

本地按改动选择文件或项目；CI 通过 `check:ci:*` 组合全量检查，复用包内和仓库脚本的检查命令。

## 使用

在仓库根按需选择，测试文件路径相对于所属包：

| 验证目标                 | 命令                                           |
| ------------------------ | ---------------------------------------------- |
| 某个测试文件／用例       | `pnpm --filter <包名> test <文件> [-t <用例>]` |
| 某个项目的类型           | `pnpm --filter <包名> typecheck`               |
| 分支影响范围内的快速测试 | `pnpm test`                                    |
| 分支影响范围内的类型检查 | `pnpm typecheck`                               |
| 桌面端 e2e               | `pnpm --filter @music/player test:e2e:desktop` |
| 安装包体积与网络权限     | `pnpm check:build`                             |
| 仓库脚本                 | `pnpm test:scripts`                            |

例如修改播放队列组件时：

```sh
pnpm --filter @music/player test src/layouts/QueuePanel.test.tsx
```

根目录 `test` 和 `typecheck` 的范围包含整个分支相对基线的改动，以及暂存、未暂存、未跟踪文件。加 `--dry` 可预览选中的任务。尚无提交或找不到默认分支时，会提示并回退到所有已配置的对应快速检查。

已通过且输入未变的检查无需重复执行。pre-commit 只执行暂存 TS 文件的轻量 lint 和暂存文件的格式化；pre-push 自动执行 `pnpm typecheck`。

## Rust 包与测试素材

Rust 包（`packages/music-core`、`packages/music-audio`、`packages/tauri-plugin-playback`）各带一个只含脚本的 `package.json`：`test` 即 `cargo test -p <crate>`，`typecheck` 即 `cargo check -p <crate> --tests`，所以上表命令和 `pnpm test` 同样覆盖 Rust。追加的参数直接交给 cargo：

```sh
pnpm --filter @music/core test --test player shuffle
```

想一次看到所有失败的测试文件时加 `--no-fail-fast`。Rust 代码用 `cargo fmt --all` 格式化（[rustfmt.toml](../rustfmt.toml)）。

播放器应用的测试和类型检查：

```sh
pnpm --filter @music/player test
pnpm --filter @music/player typecheck
```

不带参数的 `test` 依次运行组件测试和 Tauri 外壳的 Rust 测试（`apps/player/src-tauri/tests`）；带测试文件或 `-t` 时只运行对应的组件测试。类型检查同样包括前端 TypeScript 和 Rust 外壳。Windows 本地开发需要 Rust MSVC 工具链和 WebView2。

## 桌面端 e2e 与构建检查

桌面 e2e 用 [WebdriverIO](https://webdriver.io) 通过 `tauri-driver` 驱动真实窗口（`apps/player/e2e/desktop/`）：

```sh
pnpm --filter @music/player test:e2e:desktop
```

环境前提（Windows）：

- `tauri-driver`：`cargo install tauri-driver --locked`；
- `msedgedriver`：与本机 Edge/WebView2 **同版本**，从 `https://msedgedriver.microsoft.com/<版本>/edgedriver_win64.zip` 下载，
  放到 `apps/player/../.tools/msedgedriver.exe`（仓库外即可），或用 `MSEDGEDRIVER_PATH` 指定路径；
- 被测二进制：`pnpm --filter @music/player exec tauri build --debug`（内嵌前端的调试包；`tauri dev` 产物依赖未启动的 dev server，不能用于 e2e）；
- 需要可用的音频输出设备（播放相关用例）；Windows 上会弹出被测应用窗口，属正常现象。

每个用例开始时会清空 `target/e2e-data`（运行级临时数据目录，经 `MUSIC_DATA_DIR` 注入被测应用）并重建会话，用例之间互不影响。导入通过 WebDriver 在页面内调用应用自身的 `import_paths` 命令完成，参数为 `fixtures/music/` 下的文件。全部用例通过即成功；失败时 WDIO 输出会定位到具体用例。

构建检查检查安装包体积与网络能力：

```sh
pnpm --filter @music/player exec tauri build   # 生成安装包
pnpm check:build
```

`check:build` 断言 NSIS 安装包不超过 15 MB、Tauri CSP 与 capabilities 没有开放网络能力，并断言安卓 release 变体通过清单合并移除了 INTERNET 权限（`gen/android/app/src/release/AndroidManifest.xml`；开发模式的 INTERNET 保留）。

安卓构建：需要 JDK 17（`JAVA_HOME`）、Android SDK（`ANDROID_HOME`）和 NDK 27.2（`NDK_HOME`，`sdkmanager "ndk;27.2.12479018"`）。

```sh
pnpm --filter @music/player exec tauri android build --debug --target aarch64
adb install -r apps/player/src-tauri/gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk
```

Windows 需要开启[开发者模式](https://learn.microsoft.com/windows/apps/get-started/enable-your-device-for-development)以允许符号链接（tauri 把 .so 链接进 jniLibs）。Gradle 依赖下载走本地代理时，给 gradle 传 `-Dhttps.proxyHost=… -Dhttps.proxyPort=…`。安卓端的文件选择、后台与锁屏播放、通知栏控制和音频焦点目前在真机上手动验证。

测试素材在 `fixtures/music/`，由 [生成脚本](../scripts/gen-fixtures.mjs) 产出，需要 PATH 中有带 libmp3lame、libvorbis 的 ffmpeg：

```sh
node scripts/gen-fixtures.mjs
```

生成结果逐字节可重现；修改脚本后重新生成，与脚本一起提交。素材的内容（文件名、时长、频率、封面、歌词编码）以脚本为准。

## CI 与本地复现

[CI](../.github/workflows/ci.yml) 在 Windows 上使用根 [package.json](../package.json) 中的以下入口：

| 命令                        | 检查范围                                |
| --------------------------- | --------------------------------------- |
| `pnpm check:ci:static`      | 全量 lint、所有包及仓库脚本的类型检查   |
| `pnpm check:ci:unit`        | 所有包的快速测试、仓库脚本测试          |
| `pnpm check:ci:e2e-desktop` | 桌面端 e2e（构建调试包后运行）          |
| `pnpm check:ci:build`       | 构建 release 安装包并运行 `check:build` |

这些入口不按本地 Git 改动缩小范围，Turbo 仍可复用缓存。

复现 CI 失败时，在仓库根执行失败步骤中的同名命令；日志已定位具体用例时，可先运行所属包的该用例。依赖按锁文件安装，并按对应 CI 步骤准备平台、Node/pnpm 版本、环境变量和所需服务。

## 新增应用或共享包

每个包维护自己的 `package.json` 脚本，根命令会自动发现：

| 包内脚本    | 内容                                                |
| ----------- | --------------------------------------------------- |
| `test`      | 快速单测／组件测试；不启动完整应用                  |
| `typecheck` | TypeScript 项目的完整类型检查，可使用编译器增量缓存 |

无测试的包不声明 `test`，不使用 `--passWithNoTests` 隐藏漏配。workspace 依赖写进包清单，包括使用的 `@music/tsconfig`，让消费者进入影响范围。

普通 TypeScript 包复用当前入口：

```json
{
    "scripts": {
        "test": "vitest run --config ../../vitest.unit.config.ts",
        "typecheck": "tsc --noEmit"
    }
}
```

[共用单测配置](../vitest.unit.config.ts) 只发现 `src/`、`test/`、`tests/` 下的 `*.test.*`／`*.spec.*`，排除契约、集成和 e2e 目录与对应后缀。

## 新增测试层级

新增 e2e 等层级时：在所属包加脚本（如 `test:e2e:android`），在 `turbo.json` 加同名任务（e2e 设 `cache: false`），再添加对应的 `check:ci:*` 入口和 CI 步骤，并把运行命令写进本文。这些层级独立于普通 `test` 和 Git hooks。

读取包外文件的任务，在包级 `turbo.json` 的 `inputs` 中声明这些文件（保留 `$TURBO_DEFAULT$`），例如 Rust 包声明根目录的 `Cargo.toml`、`Cargo.lock` 和 `fixtures/music/**`。

## 范围与缓存

[范围脚本](../scripts/run-affected.ts) 默认依次选择 `origin/HEAD`、`origin/main`、`origin/master`、非当前分支的本地 `main`／`master`，从共同祖先比较改动。PR 目标不是默认分支时，给 `test` 或 `typecheck` 添加 `--base <ref>`，例如 `pnpm test --base origin/develop`。也可通过 `TURBO_SCM_BASE` 指定基线。

脚本不自动 fetch，指定基线须已存在于本地；显式基线错误或无法确定共同祖先时直接失败。

[Turbo 配置](../turbo.json) 按任务输入选择检查并复用缓存；类型检查还可使用 TypeScript 增量缓存。共享源码通过 `transit` 传播影响和缓存失效，不要求先跑完依赖包的测试。根脚本独立选择；纯 `docs/*.md` 说明不触发它们。锁文件、根包配置、Turbo 配置改动会扩大检查范围。

没有选中任务只表示无适用检查，不代表业务行为已验证。
