# AGENTS.md

离线本地音乐播放器：一个 Tauri 2 应用产出 Windows 桌面端和安卓端，播放与曲库逻辑在 Rust，界面是 React。

## 目录

```
project/
├── apps/
│   └── player/              # Tauri 应用：src/ 界面，src-tauri/ 外壳，e2e/ 端到端测试
├── packages/                # 内部包，桌面和安卓共用
│   ├── music-core/          # Rust：曲库、歌词、导入配对、播放模式、保存
│   ├── music-audio/         # Rust：解码、倍速与升降调、封面
│   └── request/ utils/      # TS 包，exports 直指 src 源码
├── fixtures/music/          # 各层测试共用的生成音频和歌词，进 git
├── internal/                # 配置包（tsconfig）
├── scripts/                 # 检查范围、lint 等仓库脚本
├── docs/                    # 架构、测试说明、验收清单
├── design/preview/          # 设计原型（本地，不进 git）
└── assets/                  # 开发用曲库（本地，不进 git，不进安装包）
```

## 何时读什么

| 要做的事                             | 先读                                                       |
| ------------------------------------ | ---------------------------------------------------------- |
| 改播放、曲库、歌词、导入，或新增模块 | [技术架构](docs/architecture.md)                           |
| 实现界面或判断一项功能是否完成       | [验收清单](docs/acceptance.md)，视觉对照 `design/preview/` |
| 选择或新增测试、复现 CI 失败         | [测试说明](docs/testing.md)                                |
| 修改某个具体项目                     | 所属目录的 README 和 AGENTS.md                             |

设计原型用 `node design/preview/serve.mjs` 启动，打开 `http://localhost:5178/desktop.html` 或 `mobile.html`。

## 实现边界

- 完全离线：不申请网络权限，不引入联网依赖（包括 `packages/request`）；Tauri 框架自身的传递依赖除外。
- 播放状态只在 Rust 保留一份，界面通过命令和事件投影它；平台差异只放在 `src-tauri` 和安卓插件里。
- 导入只记录文件位置，不复制文件；移出播放列表不删除文件。
- 界面按设计原型实现，偏离前先问用户。
- 测试不依赖 `assets/`，素材放 `fixtures/music/`。

## 新增项目

- 新增应用或共享包时，声明实际 workspace 依赖并接入适用的检查命令，见 [测试接入](docs/testing.md#新增应用或共享包)。
- 项目专属 lint 规则放在项目内的 `.oxlintrc.json`，通过 `extends` 继承根配置；常规 lint 不传 `--config`，保留局部规则发现。
- 项目特有的使用、测试环境和排错说明放在所属 README；仅在有额外开发约束时添加局部 AGENTS.md。

## 验证

- 测试先行：新增或修复行为时，先写能重放它的测试，再实现到通过。判断功能是否正常只看测试和实际运行结果，不以文档里的勾选为据。
- 优先运行所属包的具体测试：`pnpm --filter <包名> test <文件> [-t <用例>]`；需要验证类型时用 `pnpm --filter <包名> typecheck`。
- 需要检查整个分支的影响范围时，用 `pnpm test` 或 `pnpm typecheck`；加 `--dry` 预览范围。
- 普通应用改动只验证所属应用；共享包改动包含消费者；IPC、持久化格式改动补对应测试。
- 包内 `test` 只放快速单测／组件测试；e2e 和构建检查分开，按需执行。
- 不因 commit/push 重复已通过且输入未变的检查；pre-push 自动运行受影响项目的 typecheck。
- `check:ci:*` 用于 CI 和对应失败的本地复现。
- 回复里只报告实际运行的命令和结果；跳过、无匹配用例和缺环境都直接说明，不算通过。

需要 lint 时运行 `pnpm lint <文件或目录>`，加 `--fix` 自动修复；全量 lint 由 CI 执行。

## 文档

- 文档只写当前状态。验证结果、修复经过和带日期的记录写进回复和提交信息。
- 验收清单是临时材料：标准由用户确定，实现者只勾选和填写最终结论，不增删改写；用户最终验收后，标准由测试承接，清单删除，仍需遵守的约定并入架构说明。
- 行为或路径变化时，只同步直接相关的文档；新增测试层的命令写进测试说明。
