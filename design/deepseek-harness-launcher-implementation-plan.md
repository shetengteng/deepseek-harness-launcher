# deepseek-harness-launcher 实施计划

> 基于 [deepseek-harness-launcher 设计文档](./deepseek-harness-launcher-design.md) 与 [原型 v4](./deepseek-harness-launcher-prototype.html)，遵循 Rust + Tauri 最佳实践落地。
>
> 目标读者：项目实施者（含 AI 编码代理）。本计划拆分到可独立验收的任务粒度，按里程碑串联。

## 0. 审阅清单

实施前必读：

- [deepseek-harness-launcher-design.md](./deepseek-harness-launcher-design.md)：架构、目录、契约、安全、跨平台
- [deepseek-harness-launcher-prototype.html](./deepseek-harness-launcher-prototype.html)：UI 视觉、状态机、文案
- [deepseek-harness-desktop/apps/desktop/src/host-supervisor.ts](./deepseek-harness-desktop/apps/desktop/src/host-supervisor.ts)：就绪行解析、超时/重试语义（Rust 端要等价实现）
- [deepseek-harness-desktop/apps/desktop/src/main.ts](./deepseek-harness-desktop/apps/desktop/src/main.ts)：webview 安全策略原版

约定：

- 仓内代码与注释跟随 `deepseek-harness-desktop/AGENTS.md` 的工程风格（直接、具体、不叙述控制流）
- 每个 PR 配 Agent Note；非平凡行为配测试
- 命令、文件路径、配置项用 `code` 标注

---

## 1. 技术选型与版本基线

| 组件         | 版本                                                  | 备注                                                              |
| ------------ | ----------------------------------------------------- | ----------------------------------------------------------------- |
| Rust         | 1.80+ stable                                          | edition 2021                                                      |
| Tauri        | 2.x                                                   | 使用 `tauri` + `tauri-cli` 2.0 正式版                             |
| 前端框架     | Vue 3 + TypeScript 5                                  | `<script setup>` 单文件组件                                       |
| UI 组件      | shadcn-vue                                            | 基于 Radix Vue + Tailwind，组件源码落到 `src/components/ui/` 自管 |
| CSS          | Tailwind CSS 3                                        | shadcn-vue 依赖；暗色 token 通过 CSS 变量切换                     |
| 构建         | Vite 5                                                | Tauri 默认前端管线                                                |
| 状态管理     | Pinia                                                 | 替代 React 的 hooks 自管状态                                      |
| 包管理       | pnpm                                                  | 与 dsh 仓一致                                                     |
| State 序列化 | `serde` + `serde_json`                                | `state.json` 结构见设计 §4.3                                      |
| 子进程       | `tokio::process` + `tauri::async_runtime`             | 异步监管                                                          |
| HTTP         | `reqwest` (rustls)                                    | 跨平台、避开 OpenSSL 依赖                                         |
| Semver       | `semver` crate                                        | Node `engines` 校验与 dsh 已发布版本排序                          |
| 解压         | `flate2` + `tar`（Unix）；`zip`（Windows）            | Node tarball                                                      |
| 日志         | `tracing` + `tracing-subscriber` + `tracing-appender` | 滚动文件                                                          |
| 错误         | `thiserror`（库内）+ `anyhow`（bin 内）               | 见 §7                                                             |

Tauri 插件：

- `tauri-plugin-updater`：壳子自身升级（M5）
- `tauri-plugin-shell`：可选，辅助打开外链
- `tauri-plugin-dialog`：错误对话框、文件选择
- `tauri-plugin-fs`：受限范围，仅在需要时开权限

---

## 2. 仓库与工程结构

新建独立仓库 `deepseek-harness-launcher`（与 `deepseek-harness-desktop` 同级或同工作区，**不**作为 dsh 子目录）：

```
deepseek-harness-launcher/
├── .github/
│   └── workflows/
│       ├── ci.yml              # lint + test + build matrix
│       └── release.yml         # 签名 + 发布
├── AGENTS.md                   # 简版工程约定，引用设计文档
├── README.md
├── package.json                # pnpm workspace root，前端依赖
├── pnpm-workspace.yaml
├── src/                        # 前端源（Vue 3 SFC）
│   ├── App.vue
│   ├── main.ts
│   ├── styles.css              # Tailwind base + shadcn token (HSL 变量)
│   ├── tailwind.config.ts      # shadcn-vue 官方 preset
│   ├── components.json         # shadcn-vue CLI 配置
│   ├── pages/
│   │   ├── Main.vue
│   │   ├── FirstRun.vue
│   │   ├── Settings.vue
│   │   └── ErrorDialog.vue
│   ├── components/
│   │   ├── ui/                 # shadcn-vue 生成（Button/Dialog/Progress/...）
│   │   │   ├── button/
│   │   │   ├── dialog/
│   │   │   ├── progress/
│   │   │   ├── select/
│   │   │   ├── badge/
│   │   │   ├── card/
│   │   │   ├── input/
│   │   │   ├── label/
│   │   │   ├── switch/
│   │   │   └── toast/
│   │   ├── ProgressBar.vue     # 业务封装，基于 ui/progress
│   │   ├── VersionBadge.vue    # 业务封装，基于 ui/badge
│   │   ├── MirrorSelector.vue  # 业务封装，基于 ui/select
│   │   ├── stores/                 # Pinia
│   │   ├── launcher.ts
│   │   └── upgrade.ts
│   ├── composables/            # Vue 组合式函数
│   │   ├── useTauriEvent.ts
│   │   ├── useDshStatus.ts
│   │   └── useTrayEvents.ts    # 托盘事件，切换主窗口 / 设置视图
│   └── lib/
│       ├── tauri.ts            # invoke 封装与类型
│       ├── utils.ts            # cn() = clsx + tailwind-merge
│       └── format.ts
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── icons/
│   ├── capabilities/
│   │   └── default.json
│   └── src/
│       ├── main.rs
│       ├── lib.rs              # 便于测试 import
│       ├── commands.rs         # #[tauri::command] 暴露面
│       ├── state.rs            # AppState、state.json 读写
│       ├── error.rs            # LauncherError + Serialize
│       ├── mirror.rs
│       ├── paths.rs            # 数据/日志目录解析
│       ├── logging.rs
│       ├── node/
│       │   ├── mod.rs
│       │   ├── download.rs
│       │   ├── install.rs
│       │   └── version.rs
│       ├── dsh/
│       │   ├── mod.rs
│       │   ├── registry.rs
│       │   ├── install.rs
│       │   ├── version.rs
│       │   └── integrity.rs
│       └── host/
│           ├── mod.rs
│           ├── supervisor.rs   # 移植 host-supervisor.ts
│           ├── readiness.rs
│           └── lifecycle.rs
└── tests/
    └── e2e/                    # tauri-driver 或 webdriver 集成测试
```

**目录决策**：

- `src-tauri/src` 子模块划分严格对应设计 §6.1，避免位置漂移
- 跨平台路径解析集中在 `paths.rs`，任何模块拿目录都走它
- 前端只在 `lib/tauri.ts` 调 `invoke`，组件不直接拼命令名

---

## 3. 里程碑总览

| 阶段 | 目标                                          | 验收                                      |
| ---- | --------------------------------------------- | ----------------------------------------- |
| M1   | 最小可用：Tauri 壳子 + 系统 Node + 手动装 dsh | 能在本机跑起 dsh web                      |
| M2   | Node 托管                                     | 首启下载 Node 到用户目录，不依赖系统 Node |
| M3   | dsh 托管                                      | 自动拉取、版本切换、`known_good` 回滚     |
| M4   | 健壮性                                        | 崩溃恢复、错误提示、日志、镜像源切换      |
| M5   | 发布                                          | 跨平台 CI、签名、公证、自动更新           |

每个里程碑结束后：跑 `cargo test`、`pnpm test`、`pnpm build`，并更新 `CHANGELOG.md`。

---

## 4. M1 — 最小可用

**目标**：把 Tauri 壳子立起来，能 spawn 系统已装 Node 跑 dsh，并在 webview 里加载 dsh web。**不做** Node 下载、不做 dsh 自动安装、不做回滚。

### M1.1 工程脚手架 ✅ PR-001 已完成

- [x] 初始化 pnpm + Tauri 工程：`pnpm create tauri-app` 选 Vue + TS
- [x] 配置 `tauri.conf.json`：
  - `productName: "deepseek-harness-launcher"`
  - `identifier: "io.deepseek.deepseek-harness-launcher"`
  - `app.window.title: "deepseek-harness-launcher"`、`width: 1280`、`height: 840`、`minWidth: 960`、`minHeight: 600`
  - `app.security.csp` 仅允许 `default-src 'self'`、`connect-src` 允许 `http://127.0.0.1:*` `http://localhost:*`、`frame-src http://127.0.0.1:* http://localhost:*`、`style-src 'self' 'unsafe-inline'`（Tailwind/shadcn 内联所需）
  - 关闭 `withGlobalTauri`，使用显式 `@tauri-apps/api`
- [x] `Cargo.toml` 加入：`tokio`、`serde`、`serde_json`、`thiserror`、`anyhow`、`tracing`、`tracing-subscriber`、`reqwest`、`semver`、`directories`（用户目录解析）
- [x] `pnpm` 依赖：
  - 核心：`vue@^3.5`、`pinia@^2.3`、`@tauri-apps/api`、`@tauri-apps/plugin-dialog`（注：`radix-vue` 已重命名为 `reka-ui`，依赖名同步更新）
  - Tailwind + shadcn-vue 基座：`tailwindcss@^3.4`、`postcss`、`autoprefixer`、`reka-ui`、`class-variance-authority`、`clsx`、`tailwind-merge`、`tailwindcss-animate`、`lucide-vue-next`、`@vueuse/core@^11`
  - 构建：`@vitejs/plugin-vue`、`vue-tsc`
- [x] 初始化 shadcn-vue：`pnpm dlx shadcn-vue@latest init`，选 Default 风格 + Slate 调色板 + CSS Variables
- [x] 生成初始组件：`pnpm dlx shadcn-vue@latest add button card dialog progress select badge input label switch toast`（注：CLI 拉取 registry 受代理影响，改用 [scripts/fetch-shadcn.mjs](../scripts/fetch-shadcn.mjs) 直接 fetch 落盘）
- [x] `tailwind.config.ts`：`darkMode: ["class"]`；扩展 `keyframes` 加首启向导需要的微动效
- [x] `styles.css`：把原型 `:root` HSL token 映射到 shadcn 变量（`--background` `--foreground` `--primary` 等），保持暗色为默认（`.dark` 应用在 `<html>`）
- [x] `lib/utils.ts`：`export function cn(...inputs) { return twMerge(clsx(inputs)) }`
- [x] 配置 ESLint + Prettier（`@vue/eslint-config-typescript`），禁用未使用变量；Rust `clippy` 设为 deny warnings（依赖已装，clippy deny warnings 待 PR-002 配 `.cargo/config.toml`）
- [x] 配置 `rustfmt.toml`：`edition = "2021"`、`max_width = 100`
- [x] 写 `AGENTS.md` 简版，指向设计文档与本计划

**验收**：`cargo check` 全绿（528 包 43.64s），`pnpm tauri dev` 启动 webview 窗口（Vite 1420 端口 + Rust 二进制 PID 44768）。已知遗留：`use-toast.ts:82` TS2589 类型递归（M1.5 接入 Toast 时修）。

### M1.2 状态层（`state.rs` + `paths.rs` + `error.rs` + `logging.rs`） ✅ PR-002 已完成

- [x] `paths.rs`：用 `directories::ProjectDirs` 解析数据/日志/缓存目录，跨平台分支见设计 §4.2
  - macOS 数据目录 `~/Library/Application Support/io.deepseek/DeepSeek/deepseek-harness-launcher/`
  - macOS 壳子日志 `~/Library/Logs/deepseek-harness-launcher/`（cfg 分支）
  - 其他平台壳子日志统一落到 `<data_dir>/logs/` 与 dsh 子进程日志同根，便于打包导出
- [x] `state.rs`：定义 `AppState` 结构（对应设计 §4.3 的 `state.json`），`schema_version: u8 = 1`
  - `NodeState` / `DshState` / `InstalledDsh` 子结构，`#[serde(default)]` 让字段缺失时回退默认值
  - `StateStatus::Loaded(Box<AppState>)` box 起来避免 enum 大小差异（clippy::large_enum_variant）
- [x] 实现 `load_state` / `save_state`，写入用临时文件 + `rename` 原子替换
  - `save_to<P: AsRef<Path>>` / `load_from<P: AsRef<Path>>` 泛型化接口，方便测试传 `Arc<PathBuf>`
  - `<path>.tmp` 后缀的临时文件，`fs::rename` 在同目录下原子替换
- [x] 缺失 `state.json` 时返回 `StateStatus::FirstRun`，**不写默认空文件**（`first_run_does_not_write_file` 测试守护）
- [x] `LauncherError` 含 `StateCorrupt { path, cause }`、`Io`、`Serialization`、`UnsupportedSchemaVersion`、`StateMigration`、`PathResolve`、`LoggingInit`
  - `impl Serialize for LauncherError` 输出 `{ kind, message, data }` 三段式结构供前端解析（设计 §11.1）
- [x] `logging.rs`：`tracing` + `tracing-appender::rolling::Rotation::DAILY` 按天滚动
  - macOS 日志文件名前缀 `app`（实际文件 `app.YYYY-MM-DD`），保留期 7 天
  - debug 构建额外输出到 stderr，release 静音避免污染 GUI stdout
  - `WorkerGuard` 在 `lib::run()` 作用域保活到进程退出

**验收**：`cargo test --no-fail-fast` 20/20 通过（含并发写测试 8 线程 + 撕裂 JSON 检测）；`cargo clippy --all-targets -- -D warnings` 全绿；`.cargo/config.toml` 配 `rustflags = ["-D", "warnings"]` 让所有子命令统一 deny warnings，并加 `cargo lint` 别名。

### M1.3 Host 监管（`host/`） ✅ PR-003 已完成

参照 [host-supervisor.ts](./deepseek-harness-desktop/apps/desktop/src/host-supervisor.ts) 实现 Rust 等价物：

- [x] `readiness.rs`：
  - 常量 `READINESS_PREFIX = "dsh web: "`
  - `ReadinessParser` 结构体：`async push(chunk) -> Result<Option<Origin>>`、`async finalize() -> Result<Origin>`
  - 校验：`http:` 协议、`127.0.0.1` 或 `localhost`、显式端口 1–65535、pathname `/` 或空、无 query/hash
  - 检测冲突的就绪行（与原版 `accept` 一致，重复同 URL 幂等）
  - 手写 URL 解析避免引入 `url` crate，跨字符边界安全
  - 15 个单元测试复刻原版边界用例（split chunks / CR / query / hash / 端口 0 / 非 loopback / https / 冲突 URL / finalize tail 等）
- [x] `supervisor.rs`：
  - `HostSupervisor` 持有 `Arc<Mutex<Option<Child>>>`、`OnceCell<Origin>`、`AtomicBool` shutdown 标志
  - `start()`：spawn 子进程，stdout 通过 `oneshot` 通道回传就绪 origin，超时 90s 杀进程
  - `shutdown()`：直接 SIGKILL（`child.kill()`），幂等（child 被 take 后是 no-op）；TS 版 SIGTERM → 5s → SIGKILL 留到后续接入 nix crate
  - stdout 缓冲上限 `MAX_STARTUP_OUTPUT_CHARS = 32_768`，超出从头部按 UTF-8 字符边界截断
  - `on_unexpected_exit` 回调：通过 `shutdown_flag` 区分主动关闭与意外退出
  - exit monitor task：等 stdout/stderr EOF 后 `child.wait()` 取 exit code/signal
- [x] `lifecycle.rs`：封装 `spawnDshWeb` 等价逻辑：
  - 命令：`<node> --expose-internals <cli_entry> web --host 127.0.0.1 --port 0`
  - `stdio: [null, pipe, pipe]`，Windows `CREATE_NO_WINDOW`（等价 `windowsHide`），`kill_on_drop(true)`
  - 环境变量过滤：剥离 `RUST_*`、`TAURI_*`、`npm_*`/`npm_config_*`/`npm_lifecycle_*`/`npm_package_*`，只透传 `DSH_*`、`PATH`、`HOME`/`USERPROFILE`、`LANG`、`LC_*`
- [x] M1 阶段 `nodeExecutable` 直接读 `state.node.binary` 或回退系统 `PATH` 上的 `node`；`cliEntry` 读环境变量 `DSH_CLI_ENTRY`（开发者手动指定，未设置返回 `PathResolve` 错误）

**验收**：`cargo test --no-fail-fast` 47/47 通过（readiness 15 + lifecycle 10 + supervisor 4 + state 18）；`cargo clippy --all-targets -- -D warnings` 全绿。手动集成验证（`DSH_CLI_ENTRY=... pnpm tauri dev`）推迟到 PR-004 命令层落地后。

### M1.4 Tauri 命令层（`commands.rs`） ✅ PR-004 已完成

- [x] `#[tauri::command] async fn launcher_status() -> Result<StatusSnapshot>`：读 `state.json`，返回 `phase`（`first_run`/`idle`）、`host_origin`、`dsh_version`、`node_version`
- [x] `#[tauri::command] async fn start_host(state: State<SharedState>) -> Result<String>`：构造 `SpawnDshWebOptions`（`DSH_CLI_ENTRY` 环境变量 + `filtered_env`），调 `HostSupervisor::start`，返回 origin 字符串
- [x] `#[tauri::command] async fn shutdown_host(state: State<SharedState>) -> Result<()>`：调 `HostSupervisor::shutdown`，幂等
- [x] `SharedState` 注入 `tauri::Builder::manage()`，持有 `HostSupervisor` 实例
- [x] `LauncherError` 新增 `Host(String)` variant，`HostSupervisorError` 通过 `map_host_error` 转换，`kind_str` 返回 `"host"`，`data` 为 `None`
- [x] `lib.rs` 移除占位 `greet` 命令，注册 `launcher_status` / `start_host` / `shutdown_host`

**验收**：`cargo test --no-fail-fast` 50/50 通过（commands 3 + host 27 + state 18 + paths/error 2）；`cargo clippy --all-targets -- -D warnings` 全绿；`pnpm lint` + `pnpm build` 全绿。手动集成验证推迟到 PR-005 前端骨架落地后。

### M1.5 前端骨架 ✅ PR-005 已完成

- [x] `App.vue`：挂载 `MainView`，M1 简化不引入 Toast 容器（M2 接入首启向导时再加）
- [x] `MainView.vue`：按 phase 渲染（booting → Loading spinner；first_run → Card + 启动按钮；idle → 版本信息 + 启动 Host 按钮；ready → iframe 加载 origin；error → ErrorDialog）
  - 选 iframe 方案便于隔离；csp 已在 M1.1 放行 `frame-src http://127.0.0.1:*`
  - `sandbox="allow-same-origin allow-scripts allow-forms allow-popups allow-modals"`
- [x] `ErrorDialog.vue`：基于 shadcn `Dialog`，展示 `{ kind, message, data }` + 重试/关闭按钮，`data` 用 `<pre>` 展示
- [x] Pinia store `stores/launcher.ts`：`phase`/`origin`/`dshVersion`/`nodeVersion`/`error`/`starting`/`stopping`，actions：`refreshStatus`/`startHost`/`shutdownHost`/`resetError`，并发守卫（`starting`/`stopping` 标志位）
- [x] `lib/tauri.ts`：`invokeCommand` 封装 `invoke`，统一错误类型 `LauncherErrorPayload`，导出 `fetchStatus`/`startHost`/`shutdownHost`
- [x] vitest 配置 + 24 个前端单测（store 13 + MainView 6 + ErrorDialog 5）
- [x] 补齐 PR-004 推迟的 commands 集成测试（`tests/commands_integration.rs`，4 个用例：shutdown 幂等 / AlreadyShutdown / 错误 message / 默认超时合理性）

**未实现（推迟到后续 PR）**：

- macOS 风格 titlebar（推迟到 M3 前端设置页一起做）
- `composables/useTauriEvent.ts`（M2 接入首启向导时按需写）
- 系统托盘与窗口生命周期（M4.5；原型中的托盘视觉作为菜单信息架构参考，实际呈现遵循各平台原生菜单）
- `tauri::RunEvent::ExitRequested` → shutdown 联动（M4 崩溃恢复一起做）

**验收**：`cargo test` 59/59（commands 8 + host 27 + state 18 + 其他 2 + 集成 4）；`pnpm test` 24/24；`cargo clippy -- -D warnings` + `pnpm lint` + `pnpm build` 全绿。手动集成验证推迟到 M2 首启向导落地后。

---

## 5. M2 — Node 托管

**目标**：首启自动下载 Node 到用户目录，之后所有 spawn 走托管 Node，不依赖系统 PATH。

### M2.1 镜像源（`mirror.rs`） ✅ PR-007 已完成

- [x] 镜像源表（设计 §8.4）：`npmmirror.com/mirrors/node`（国内优先）、`nodejs.org/dist`、`mirrors.tuna.tsinghua.edu.cn/nodejs-release`
- [x] `Mirror` 结构：`id`（`MirrorId` 枚举：`NodejsOrg`/`Npmmirror`/`Tuna`/`Custom(String)`）、`name`、`base_url`、`trusted: bool`
- [x] `pick_default_mirror()`：读 `LANG`/`LC_ALL`/`LC_MESSAGES`，zh* 前缀返回 npmmirror，否则 nodejs.org
- [x] `validate_custom_mirror(raw)`：必须 `https://` 前缀、拒绝 query/fragment、归一化尾斜杠、`trusted: false`
- [x] 探活：`probe_mirror(client, mirror, timeout)` GET `{base_url}/index.json`，200 → Ok；超时 → `ProbeTimeout`；连接拒绝 → `ProbeNetwork`；其他非 200 → `ProbeFailed`
- [x] `probe_mirrors(client, mirrors, timeout)`：顺序探活，首个 200 胜出，全失败返回 `AllMirrorsFailed { tried }`
- [x] `Mirror` 提供 `index_url` / `archive_url(version, platform, arch)` / `shasums_url(version)` 三个 URL 拼接方法
- [x] `LauncherError` 新增 `Mirror(String)` variant，`MirrorError` 通过 `From` 转换，`kind_str` 返回 `"mirror"`
- [x] `node/mod.rs` re-export 公开 API
- [x] 单元测试 14 个：内置源数量、URL 拼接、archive/shasums URL、join_url 尾斜杠、validate 拒绝 http/裸域名/query/fragment、validate 接受 https + 去尾斜杠、pick_default 三个分支、MirrorId Display
- [x] 集成测试 9 个（`tests/mirror.rs` + wiremock）：probe 200/404/500/超时/连接拒绝、probe_mirrors 首个 ok/回退/全失败、validate 集成验证

**验收**：`cargo test` 82/82（lib 69 + commands_integration 4 + mirror 9）；`cargo clippy -- -D warnings` + `pnpm lint` + `pnpm build` 全绿。

### M2.2 下载与校验（`node/download.rs`） ✅ PR-008 已完成

- [x] `download_archive(client, url, dest_path, progress_tx)`：流式 GET，写到 `.part` 临时文件，校验通过后 rename，避免中途崩溃留下半成品
- [x] 进度事件 `ProgressEvent { stage, bytes, total }`：每 64KB 推一次，避免 channel 过载；`stage` 含 `download` / `verify`
- [x] `fetch_shasums(client, mirror, version)`：拉 `SHASUMS256.txt` 全文
- [x] `parse_shasums_line(line)`：解析 `<hash>  <filename>`，校验 hash 为 64 位十六进制；`find_sha_in_shasums` 按文件名查找
- [x] `verify_sha256(path, expected_sha)`：流式 SHA-256，不匹配则删除文件
- [x] `download_with_retry(client, mirror, version, archive_filename, dest_dir, progress_tx, max_retries)`：失败重试，单镜像最多 `max_retries` 次，全失败抛 `NodeDownload` 错误
- [x] `LauncherError` 新增 `NodeVersion(String)` 和 `NodeDownload(String)` variant
- [x] 单元测试 18 个：shasums 解析/前缀空格/短 hash/非 hex/查找、SHA-256 匹配/不匹配删除、ProgressEvent 序列化、parse_node_version 6 种输入、satisfies_engines 5 种 range、current_node_satisfies 三分支、DEFAULT_NODE_VERSION 自检
- [x] 集成测试 9 个（`tests/download.rs` + wiremock + 真实 tar.gz fixture）：
  - 真实 `node-v22.19.0-darwin-arm64-arm64.tar.gz` fixture（含 `bin/node`、`bin/npm`、`package.json`），用 `tar::Builder` + `flate2::GzEncoder` 动态生成，并计算真实 SHA-256 拼装 SHASUMS256.txt
  - 覆盖：200+SHA 通过、404 错误、5xx 重试 3 次后 Err、SHA 不匹配 + 文件删除、进度事件触发（200KB 大文件）、重试一次后成功、SHASUMS 缺失 404、真实 archive 校验

**验收**：`cargo test` 115/115（lib 93 + commands_integration 4 + mirror 9 + download 9）；`cargo clippy -- -D warnings` + `cargo fmt --check` 全绿。

- `GET {mirror}/v{version}/node-v{version}-{platform}-{arch}.tar.gz`
- 流式下载，`reqwest::Response::bytes_stream()` → 写临时文件
- 通过 `tauri::Emitter::emit` 推 `download-progress` 事件（`{ stage, bytes, total }`）
- [ ] SHA-256 校验：`GET {mirror}/v{version}/SHASUMS256.txt`，匹配同名条目
- [ ] 失败重试：单镜像最多 2 次，全镜像失败抛 `NodeDownloadExhausted`
- [ ] macOS 下载完成后调用 `xattr -d com.apple.quarantine`（Rust 用 `std::process::Command` 或 `xattr` crate）

### M2.3 解压与安装（`node/install.rs`）

- [ ] 解压目标：`<data_dir>/node-runtime-new/`
- [ ] Unix：`flate2::GzDecoder` + `tar::Archive`，保留可执行位
- [ ] Windows：`zip::ZipArchive` 解压
- [ ] 校验入口：`node-runtime-new/bin/node`（或 `node.exe`）存在并能 `--version` 输出预期版本
- [ ] 原子切换：`rename(node-runtime-new, node-runtime)`，原目录存在则先 `rename(node-runtime, node-runtime-old)`，切换后删 old
- [ ] 写 `node-runtime/VERSION`
- [ ] 更新 `state.node`

### M2.4 版本与 engines（`node/version.rs`）

- [ ] `parse_node_version(s) -> Result<Version>`
- [ ] `satisfies_engines(node_version, engines_node_range) -> bool`：解析 dsh `package.json.engines.node`，用 `semver::VersionReq`
- [ ] 默认目标版本：硬编码常量 `DEFAULT_NODE_VERSION = "v22.19.0"`（与 dsh engines 对齐，每发布前确认）

### M2.5 首启向导（`FirstRun.vue`） ⚠️ PR-011 已完成，交互由 PR-020b 替换

PR-011 已实现 Node 下载、镜像选择和进度事件基础设施，但其“先选镜像 → 下载安装 Node → 手动安装 dsh → 手动启动”的交互不再是目标流程。保留其后端下载、校验、解压、镜像和事件能力；首启 UI、状态机及自动编排以 PR-020b 为准。

#### 已完成基础设施

- [x] 后端 Tauri 命令：
  - `list_mirrors`：返回内置镜像源 `MirrorInfo[]`
  - `probe_mirrors_command(custom_urls?)`：探活后返回首个可用源
  - `validate_custom_mirror_command(url)`：校验自定义源（必须 https、无 query/fragment）
  - `install_node_command(args)`：下载 + 校验 + 解压 + 原子切换 + 写 state.json，通过 `download-progress` / `extract-progress` 事件推送 `ProgressEvent`
  - `fake_install_node_command(version)`（debug only）：跳过下载直接写 VERSION + state
- [x] 前端 store 扩展（`stores/launcher.ts`）：
  - 新增子状态机 `wizardStep = mirror_select | probing | downloading | extracting | done | failed`
  - `loadMirrors()` / `autoPickMirror()` / `validateCustomMirror()` / `selectMirror()`
  - `installNode({useFake, version})`：调用 Tauri 命令，监听事件更新进度
  - `applyProgressEvent(ev)`：区分 download/extract 阶段，extract complete 用 `total === 0` 判断
  - 计算属性 `selectedMirror` / `downloadPercent`
  - `detectPlatformArch()`：按 UA 推断 `darwin-arm64` / `linux-x64` / `win-x64`
- [x] 镜像源选择器（`components/MirrorSelector.vue`，shadcn-vue）：
  - 内置源 `Select` 下拉，自定义源 `Input` + debounce 300ms 即时校验
  - 校验通过自动选中，失败显示错误图标
  - "自动选择最快源"按钮触发探活
- [x] 首启向导主组件（`components/FirstRun.vue`）：
  - 4 步流程：镜像源选择 → 下载中 → 解压中 → 完成
  - `Progress` 条显示下载百分比和解压进度
  - 失败页显示错误信息 + 重试按钮
  - 完成页显示"启动 dsh"按钮，调用 `store.startHost()`
  - 监听 Tauri 事件 `download-progress` / `extract-progress`
- [x] MainView 集成：`first_run` phase 渲染 `FirstRun` 组件（替换 M1 占位 Card）
- [x] dev 按钮"[dev] 假安装"调用 `fake_install_node_command`，便于本地测试
- [x] 单元测试 7 个（`commands.rs`）：`MirrorInfo::from` 转换、序列化 snake_case、`list_mirrors` 返回内置源、`validate_custom_mirror_command` 接受 https / 拒绝 http / 拒绝 query
- [x] store 测试 14 个：wizard 初始状态、loadMirrors 默认选中、selectMirror、validateCustomMirror 成功/失败/空、selectedMirror computed、downloadPercent computed、applyProgressEvent、installNode 成功/失败/无镜像源/并发保护、resetWizard、autoPickMirror 成功/失败
- [x] 组件测试 8 个（`FirstRun.test.ts`）：挂载显示选择器+下载按钮、downloading UI、extracting UI、done UI、failed UI + 重试、禁用按钮、dev 假安装触发
- [x] MainView 测试更新：first_run 渲染向导（替换 DSH_CLI_ENTRY 占位）

**历史验收（PR-011 基础设施）**：

- `pnpm test` 56/56（launcher store 37 + ErrorDialog 5 + MainView 6 + FirstRun 8）
- `cargo clippy -- -D warnings` + `cargo fmt --check` + `pnpm lint` 全绿
- 手动验证：删除 `~/Library/Application Support/deepseek-harness-launcher/state.json` 后启动应用，进入首启向导，点"[dev] 假安装"按钮，state.json 写入 Node v22.19.0，phase 切到 idle，可启动 Host

---

## 6. M3 — dsh 托管

**目标**：自动拉取 dsh、版本切换、`known_good` 回滚。这是设计文档的**核心目标**。

### M3.1 Registry 查询（`dsh/registry.rs`） ✅ PR-012 已完成

- [x] 类型：
  - `DistTags { latest: String, others: Map }`：解析 npm `dist-tags` 对象（保留 latest + 其他 tag）
  - `DistInfo { integrity: String, tarball: String }`：单版本的 `dist` 字段
  - `EnginesField { node: String, others: Map }`：`engines.node` 范围，缺失默认空字符串
  - `PackageManifest { version, engines, dist }`：单版本完整 manifest
  - `PackageMetadata { name, dist_tags, versions }`：完整包元数据 + `manifest_for(version)` + `all_versions()`
- [x] URL 拼接：`metadata_url(registry)` → `{registry}/@deepseek-ai/dsh`；`manifest_url(registry, version)` → 单版本端点
- [x] 内置 registry 常量：`DEFAULT_REGISTRY_NPMJS = "https://registry.npmjs.org"`、`DEFAULT_REGISTRY_NPMMIRROR = "https://registry.npmmirror.com"`（新安装默认 npmjs，不覆盖用户既有 registry 设置）
- [x] 包名常量：`DSH_PACKAGE_NAME = "@deepseek-ai/dsh"`（scope 写死）
- [x] `fetch_package_metadata(registry, cache, client) -> Result<PackageMetadata>`：
  - 先查 `RegistryCache`（5min TTL），命中直接返回
  - miss 时 `GET {registry}/@deepseek-ai/dsh`
  - 校验响应 `name` 字段必须等于 `DSH_PACKAGE_NAME`，否则 `DshRegistry` 错误
  - HTTP/JSON/包名失败一律 `Err`，**不写缓存**（避免污染）
- [x] `fetch_dist_tags(registry, cache, client) -> Result<DistTags>`：复用 `fetch_package_metadata`
- [x] `fetch_package_manifest(registry, version, cache, client) -> Result<PackageManifest>`：复用 metadata + `manifest_for`，版本不存在错误信息列出所有可用版本
- [x] `fetch_package_manifest_with_client(registry, version, client)`：直连单版本端点（不缓存），用于用户明确选了某个版本
- [x] `default_client()`：10s 超时 + rustls + UA `deepseek-harness-launcher/0.1 (registry)`
- [x] 错误类型扩展：`LauncherError::DshRegistry(String)`，kind = `dsh_registry`
- [x] 单元测试 12 个（`registry.rs`）：URL 拼接、metadata 解析、manifest_for 成功/失败/缺 engines、包名校验、all_versions、缓存 put/get/invalidate/TTL 过期
- [x] 集成测试 14 个（`tests/registry.rs`，wiremock fixture）：
  - 查询 `@deepseek-ai/dsh` 返回版本列表（3 个版本）
  - dist-tags latest + next 都正确
  - 缓存命中不发 HTTP（`up_to_n_times(1)` + 第二次调用）
  - 缓存 invalidate 强制刷新（`expect(2)`）
  - fetch_package_manifest 复用 metadata 缓存
  - HTTP 404 / 500 / 错误 JSON / 包名错 都不污染缓存
  - 不同 registry 独立缓存条目
  - 单版本端点成功 + 404 + 版本不匹配
- [x] 本地演示脚本 `examples/registry_demo.rs`：起 mock 服务器，6 步流程可视化展示缓存命中（步骤 1 耗时 5ms vs 步骤 2 耗时 30µs）

**验收**：`cargo test` 162/162（lib 119 + commands_integration 4 + mirror 9 + download 9 + install 7 + registry 14）；`cargo clippy -- -D warnings` + `cargo fmt --check` 全绿；`cargo run --example registry_demo` 输出缓存命中对比。

### M3.2 安装（`dsh/install.rs`） ✅ PR-013 已完成

- [x] 类型与参数：
  - `InstallDshOptions { version, registry, dsh_dir, node_executable, npm_script, log, timeout_secs }`
  - `LogCallback = Arc<dyn Fn(&str) + Send + Sync>`：每行 npm 输出回调（tracing target=`dsh_install`）
  - `default_log()`：默认回调，tracing 写到日志文件
  - 手写 `Debug`（避免 `dyn Fn` 不实现 `Debug`），`Clone` 用 derive
- [x] `write_package_json(opts)`：写 `dsh/<version>/package.json`，内容 `{"name":"deepseek-harness-launcher-host","private":true,"dependencies":{"@deepseek-ai/dsh":"<version>"}}`，最小化无 devDependencies
- [x] `run_npm_install(opts)`：
  - cwd = `dsh/<version>/`，package.json 缺失报错
  - 命令：`<node> <npm-cli.js> install --prod --registry=<mirror> --no-audit --no-fund --loglevel info`
  - 或回退 `npm install ...`（不推荐，launcher 应自包含 npm）
  - stdout/stderr 分别 tokio::spawn 按行读取 → log callback
  - 超时控制：`tokio::time::timeout`，超时 kill 子进程
  - Windows 加 `CREATE_NO_WINDOW`
  - env_clear + 透传 PATH/HOME/USERPROFILE（剥离 DSH__、TAURI__、npm_*）
  - 退出码非 0 → `Err(DshInstall("npm install failed: exit code N"))`
- [x] `verify_install(opts)`：调用 `integrity::verify_entry_exists` 校验 `node_modules/@deepseek-ai/dsh/lib/bin.js`
- [x] `install_dsh(opts)` 完整流程：write_package_json → run_npm_install → verify_install
  - 任一步失败：删 `dsh/<version>/` 目录，错误信息附加 `(version dir cleaned: ...)`
- [x] `cli_entry(version_dir)` / `read_cli_entry(version_dir)`：计算 / 读取 CLI 入口
- [x] `options_from_manifest(manifest, registry, dsh_dir, node_dir)`：从 manifest 构造 opts，npm_script 用 `node::install::node_npm_path`

### M3.2.1 完整性校验（`dsh/integrity.rs`） ✅

- [x] `verify_entry_exists(dsh_module_dir)`：检查 `lib/bin.js` 存在
- [x] `parse_integrity(integrity)`：解析 `sha512-<base64>` 字段，非 sha512 报错
  - 手写 base64 解码（避免新增 `base64` crate 依赖）
  - 返回 `("sha512", Vec<u8>)`，避免生命周期问题
- [x] `sha512(data) -> Vec<u8>`：计算 SHA-512 二进制 digest
- [x] `verify_tarball_integrity(tarball_bytes, integrity)`：完整 SHA-512 比对，不匹配报 "mismatch"
- [x] `verify_installation(dsh_module_dir, tarball_bytes, integrity)`：入口 + tarball 双重校验

### M3.2 测试

- [x] 单元测试 11 个（`install.rs`）：路径计算、package.json 写入 / 覆盖、verify_install 成功 / 失败、cli_entry 路径、read_cli_entry 成功 / 失败、run_npm_install package.json 缺失报错、install_dsh 失败清理、install_dsh 校验失败清理、default_log 不 panic、options_from_manifest
- [x] 单元测试 10 个（`integrity.rs`）：parse_integrity 成功 / 非 sha512 / malformed、verify_tarball 匹配 / 不匹配、verify_entry 存在 / 缺失、verify_installation 组合通过 / 入口失败 / 完整性失败、npm 真实格式 round-trip
- [x] 集成测试 12 个（`tests/install_dsh.rs`，mock npm-cli.js + 真实 node）：
  - 完整流程成功（package.json + spawn + 校验）
  - package.json 内容正确
  - log callback 收集 npm 输出
  - npm install 失败清理版本目录
  - verify 失败清理版本目录
  - 部分安装失败清理
  - 创建 node_modules 结构正确
  - 不存在的 node 报错
  - 超时杀进程（1s 超时）
  - run_npm_install package.json 缺失
  - 多版本共存
  - 同版本重复安装覆盖
- [x] 本地演示脚本 `examples/install_dsh_demo.rs`：7 步流程可视化（成功路径 + 失败清理 + 超时杀进程）

**验收**：`cargo test` 201/201（lib 146 + commands 4 + mirror 9 + download 9 + install_dsh 12 + registry 14 + 其他 7）；`cargo clippy -- -D warnings` + `cargo fmt --check` 全绿；`cargo run --example install_dsh_demo` 输出完整流程日志。

### M3.3 版本切换与回滚（`dsh/version.rs`） ✅ PR-014 已完成

- [x] 错误类型扩展：`LauncherError::DshVersion(String)`，`kind_str = "dsh_version"`
- [x] `current` / `known-good` 指针：
  - Unix：`std::os::unix::fs::symlink` + `rename` 原子替换
  - Windows：写 `current.json` / `known-good.json` 指针文件，含 `version` + `updated_at`
- [x] `read_current_pointer(dsh_dir)` / `read_known_good_pointer(dsh_dir)`：读取指针指向的版本
- [x] `write_current_pointer(dsh_dir, version)` / `write_known_good_pointer(dsh_dir, version)`：原子写入指针
- [x] `switch(dsh_dir, version)`：仅改文件系统指针（不修改 state）
  - 校验 `dsh/<version>/` 存在
  - 原子替换 current 指针
- [x] `promote_to_current(state, dsh_dir, version)`：state + 指针双更新
  - 旧 current → known_good（指针 + state）
  - 新版本 → current（指针 + state）
  - 清除 pending
  - upsert installed（status = "verified"）
  - 同版本连续 promote 不会把 current 设为 known_good
- [x] `rollback_to_known_good(state, dsh_dir) -> Result<String>`：
  - 校验 known_good 存在且目录存在
  - known_good → current（指针 + state）
  - **known_good 字段清空**（已提升为 current，没有更老的 known_good）
  - 失败版本（原 current）→ ignored_versions（去重）
  - installed 中标记 broken
  - 清除 pending
- [x] `set_pending(state, version)` / `clear_pending(state)`：pending 字段管理
- [x] `ignore_version(state, version)`：加入 ignored_versions（幂等，去重）
- [x] `prune_old_versions(state, dsh_dir, keep_extra) -> Result<Vec<String>>`：
  - 保留集：current + known_good + pending + 最新 `keep_extra` 个（按版本号降序）
  - **从文件系统扫描版本目录**（不依赖 state.dsh.installed，避免 state 与 FS 不同步）
  - 跳过指针文件（current / known-good / *.json）
  - 删除不在保留集内的版本目录
- [x] `list_installed_versions(dsh_dir) -> Result<Vec<String>>`：列出所有版本（排序，排除指针）
- [x] `is_version_installed(dsh_dir, version) -> bool`：版本目录存在性检查

### M3.3 测试

- [x] 单元测试 29 个（`version.rs`）：指针读写 roundtrip、覆盖、switch、promote 首次 / 旧变 known_good / 清 pending / 失败、rollback 成功 / 无 known_good / 目录缺失 / 去重、set/clear pending、ignore 幂等、prune 保留 current+known_good / 保留 extra / 保留 pending / 跳过指针 / 空目录、list 排序 / 排除指针 / 空目录、is_version_installed、完整升级回滚场景、多次回滚累积 ignored、同版本 promote 无自引用
- [x] 集成测试 19 个（`tests/version_dsh.rs`，mock 版本目录 + state 持久化）：
  - 完整升级生命周期成功
  - 升级失败触发回滚
  - 多次升级失败累积 ignored
  - 无 known_good 时回滚报错
  - known_good 目录缺失时回滚报错
  - switch 仅改指针不改 state
  - switch 未安装版本报错
  - prune 删除旧版本
  - prune 保留 N 个额外版本
  - prune 保留 pending
  - prune 不删指针文件
  - list 返回所有版本（排序）
  - ignore 幂等
  - clear_pending
  - 完整场景：升级→失败回滚→再升级→清理
  - state 序列化/反序列化 roundtrip
  - 同版本 promote 两次不产生自引用 known_good
  - state save + reload
  - 原子指针切换一致性
- [x] 本地演示脚本 `examples/version_dsh_demo.rs`：9 步流程可视化
  - 首次安装 → 升级成功 → 升级失败回滚 → 再升级成功 → 主动忽略 → 清理旧版本 → 状态总结 → 手动 switch → clear_pending

**验收**：`cargo test` 249/249（lib 175 + commands 4 + mirror 9 + download 7 + install_dsh 12 + registry 14 + version_dsh 19 + 其他 9）；`cargo clippy -- -D warnings` + `cargo fmt --check` 全绿；`cargo run --example version_dsh_demo` 输出完整升级回滚流程。

### M3.3a 启动流程闭环修复（历史记录，首启交互由 PR-020b 替换） ✅ PR-014a 已完成

**背景**：在把 deepseek-harness-launcher 推到本机冒烟测试阶段时暴露 4 个阻塞性问题：

1. `start_host` 启动失败：`could not resolve cli_entry directory: DSH_CLI_ENTRY environment variable is not set`
   - 原因：旧 `build_spawn_options` 依赖环境变量 `DSH_CLI_ENTRY` 推导 `cli_entry`，但壳子进程没有这个变量，且未考虑 dsh 未安装场景
2. 关闭错误对话框后进入 `idle ↔ first_run` 视图循环
   - 原因：`applySnapshot` 在 Node 安装完成后立刻把 phase 切回 `idle`，覆盖了 FirstRun 的 `done` 步骤；`resetError` 无条件切回 `idle`，dsh 未装时再次 `startHost` 触发 `DshNotInstalled` 错误
3. 镜像源选择器下拉框不含"自定义"选项
   - 原因：下拉框只有内置镜像源 ID，没有"自定义"特殊值；输入框一直显示导致用户操作路径不清晰
4. 前端开发用"假安装"按钮残留（`[dev] 假装 Node 已安装`）
   - 原因：联调时为绕过真实下载流程加的旁路按钮和 `fake_install_node_command`，未清理

**修复点**：

- [x] **修复 1**：`build_spawn_options` 从 `state.node.version` 解析 `node-runtime` 目录，从 `dsh/current` 指针读 `cli_entry`，未安装时返回 `DshNotInstalled` / `NodeNotInstalled` 错误，删除 `DSH_CLI_ENTRY` 环境变量依赖（[commands.rs](../src-tauri/src/commands.rs) `build_spawn_options`）
- [x] **修复 2**：
  - `applySnapshot`：仅在当前 phase 不是 `first_run` 时才切到 `idle`，FirstRun 完成后保持 `done` 视图直到用户主动点"启动 dsh"
  - `resetError`：根据错误 `kind` 决定下一步，`node_not_installed` / `dsh_not_installed` → 切回 `first_run`（保持 `wizardStep="done"`，让用户能看到"安装 dsh"按钮），其他错误 → 切回 `idle`
- [x] **修复 3**：[MirrorSelector.vue](../src/components/MirrorSelector.vue) 下拉框增加 `<SelectItem :value="CUSTOM_VALUE">自定义...</SelectItem>`，仅选中自定义时显示输入框；选中内置源时清空 `customMirrorUrl`
- [x] **修复 4**：删除 Rust 端 `fake_install_node_command`、`lib.rs` 注册；删除前端 `lib/tauri.ts` `fakeInstallNode`、`store.installNode` 的 `useFake` 参数、`FirstRun.vue` 的"假安装"按钮
- [x] **新增**：`install_dsh_command` Tauri 命令（[commands.rs](../src-tauri/src/commands.rs)）
  - 流程：`AppState::load` → 读 `state.node` → 解析 `node-runtime` → `fetch_dist_tags` 拿 `latest` → `fetch_package_manifest` → `options_from_manifest` → `install_dsh`（npm install + verify）→ `promote_to_current` → `state.save`
  - 前端：`installDsh()` 包装、`store.installDsh()` action、`installingDsh` 状态位
  - UI：
    - `MainView.vue` idle 视图：`dshVersion === null` 显示"安装 dsh"按钮，否则显示"启动 Host"
    - `FirstRun.vue` done 步骤：`!store.dshVersion` 显示"安装 dsh"，否则显示"启动 dsh"
- [x] **错误类型扩展**：`LauncherError::DshNotInstalled { reason }` / `LauncherError::NodeNotInstalled { reason }`，`kind_str` 为 `dsh_not_installed` / `node_not_installed`，`data` 包含 `{what, reason}`，前端 `resetError` 据此切换 phase
- [x] **Rust 错误类型修正**：`LauncherError::PathResolve.what` 字段为 `&'static str`，调用处用字面量 `"dsh_current_pointer"` 而非 `.to_string()`

**验收**：

- `cargo check` 通过（无 warning）
- `pnpm exec vue-tsc --noEmit` 通过（无类型错误）
- 启动应用 → 首启向导 → 选镜像源 → 下载 Node → 显示"安装 dsh"按钮 → 点击安装 → 完成后显示"启动 Host" → 启动成功进入 dsh web

**后续策略**：这段历史实现中的后台检查、自动下载与升级提示已由 PR-020c 收敛为仅更新当前 `latest`；`Settings.vue` 的通用设置布局继续复用。

### M3.3b 首启体验与原型 03 对齐 ✅ PR-020b

**目标**：把现有首启能力重组为原型 03 的专用 bootstrap 界面。dsh 任务卡显示当前 registry `latest` 的精确版本，用户点击后才安装；不提供历史版本选择、范围规则或后台自动 dsh 升级。

- [x] 后端首启编排：
  - `get_latest_dsh_version_command` 查询 registry 元数据，验证 `dist-tags.latest` 对应 manifest 后返回精确版本
  - `resolve_bootstrap_plan_command()` 无入参，只把用户确认时的 `latest` 解析为精确 manifest、Node 目标并持久化 `bootstrap_plan`
  - `install_node_command` 完成后将计划标记为 `node_installed`；`install_dsh_command` 只消费冻结的版本和 registry，不得在重试时重新读取 `latest`
  - dsh 完整性校验成功后清除计划并自动调用 `start_host`；Host 就绪后进入 dsh Web UI
  - `state.json` schema 迁移到 `3`：保留 Node、current、known_good、pending 和已安装版本；移除范围、轮询、自动升级和忽略版本字段
- [x] 前端双任务 bootstrap 界面：
  - 采用原型 03 的标题栏、标识、说明与两个任务卡，不展示通用 Card 向导和“步骤 N/M”计数
  - dsh 任务卡显示 `latest` 解析出的精确版本和“开始安装”按钮；加载失败时显示可操作错误和重试
  - 点击后冻结目标版本；Node 卡展示实际解析版本及状态：解析、下载、SHA-256 校验、解压、完成
  - dsh 卡展示冻结的精确版本及状态：等待 Node、npm install、完整性校验、完成；仅在 npm 可报告可靠总量时展示百分比或剩余时间
  - 下载源自动选择可信镜像；仅在对应任务失败时提供“重试 / 更换镜像源”恢复操作，`MirrorSelector` 不再占据首屏
  - 删除“我已有 Node，手动指定路径”原型选项，不新增系统 Node 探测或路径配置
- [x] 错误与恢复：
  - registry 解析、Node 下载/校验、dsh 安装错误均定位到对应任务卡，并复用冻结计划重试
  - Node 成功、dsh 失败时保留 Node 成功状态；重试只继续 dsh，不重复下载 Node
  - 关闭并重开应用时读取 `bootstrap_plan`，恢复同一精确版本的未完成任务
- [x] 测试与验收：
  - Rust：`latest` manifest 校验、计划冻结/复用、Node engines 选择与 schema 迁移
  - Vue：显示当前 `latest`、开始安装、双任务状态展示、任务级错误重试
  - 手动：清除 `state.json` 后启动，确认显示最新精确版本；断网/安装失败后重试不改变已冻结版本

### M3.4 显式安装最新 dsh ✅ PR-020c

**目标**：以用户明确点击“更新到最新版本”替代后台更新策略。应用只显示 registry 当前 `latest` 的精确版本，不提供历史版本列表或选择器。

- [x] 后端：
  - `install_dsh_command` 在首启复用冻结计划，其他场景仅解析当前 `latest`，并复用 `engines.node` 校验、`install_dsh`、完整性校验和 `set_pending`
  - 当前版本就是 `latest` 时前端禁用更新；安装完成且版本已存在时仍只设 `pending`
  - 删除定时检查、候选计算、自动安装、自动重启、忽略版本以及相关 state、Tauri 命令和测试；日常启动不请求 registry
  - Node 不兼容时不写 `pending` 或 `current`，并返回可操作错误
- [x] 前端：
  - 设置页保留当前、已验证和待切换版本徽章，展示当前 `latest`、刷新按钮和“更新到最新版本”按钮
  - 用户未点击时不下载；安装失败保留显示的当前版本，并提示用户检查网络或 npm 下载源
  - registry 请求失败时保留当前运行版本，并提供刷新；不阻塞 dsh 启动
- [x] 可行性验证：
  - `dsh/registry.rs` 已有 `dist-tags.latest`、精确 manifest、缓存和错误处理，无需排序或返回完整版本列表
  - `dsh/install.rs` 已按精确版本创建隔离目录，`dsh/version.rs` 已支持 `pending`、`current`、`known_good` 和回滚；无需变更安装目录协议
  - `semver` crate 仅用于 Node `engines` 校验，不再解释或排序用户提供的 dsh 版本
- [x] 测试与验收：
  - Rust：`latest` 精确 manifest 校验、Node 不兼容时不写 pending、安装失败不提升 current、回滚
  - Vue：展示/刷新最新版本、同版本禁用更新、显式安装、网络错误时保留当前版本
  - 手动：registry 发布新版本后，应用不主动下载；用户刷新、确认更新并重启后才切换

### M3.5 既有升级功能的收敛

PR-015 与 PR-016 的版本范围输入、轮询检查、自动下载、自动重启和“发现更新”对话框不再是产品功能。PR-020c 已删除相应的状态字段、Tauri 命令、Pinia action、事件监听和 UI 控件；保留 registry 查询、安装进度、版本指针与回滚实现。历史 PR 记录仅用于追溯，不代表当前需求。

**验收**：

- 设置页默认显示 `latest（<当前解析版本>）`，并允许选择任何可安装的精确版本
- 没有用户操作时，启动应用或等待 24 小时均不发起 dsh registry 请求、下载或重启
- 选择和安装新版本后，仍可在失败时回滚到 `known_good`

---

## 7. M4 — 健壮性

### M4.1 崩溃恢复（`host/lifecycle.rs`）

- [ ] `crash_counter` 持久化到 `state.json`
- [ ] dsh 启动后意外退出（`onUnexpectedExit`）：
  - `crash_counter += 1`，写 state
  - 若 `< crash_retry_limit` 且距上次崩溃 < 5 分钟 → 自动重启 current
  - 若 `>= crash_retry_limit` → emit `crash-limit-reached` 事件，前端弹窗
- [ ] 弹窗选项：[回滚 known_good] [继续重试 current] [退出]
- [ ] 用户手动重启 app → `crash_counter = 0`

### M4.2 Node 升级流程

实现设计 §5.4：

- [ ] dsh 新版 `engines.node` 不满足当前 → 弹窗显示"需要 Node X，当前 Y"
- [ ] 用户确认 → 走 M2.2/M2.3 下载新 Node → 原子切换 → 继续 dsh 启动
- [ ] 取消则放弃该 dsh 版本升级

### M4.3 错误提示与日志

- [ ] `error.rs`：每种错误对应设计 §11.1 的用户文案
- [ ] `logging.rs`：
  - `tracing_subscriber::fmt` + `tracing_appender::rolling::daily`
  - macOS 日志目录 `~/Library/Logs/deepseek-harness-launcher/app.log`
  - dsh 子进程日志按 `<data_dir>/logs/dsh-<timestamp>.log` 分文件
- [ ] "导出诊断信息"按钮：把壳子日志 + 最近 3 份 dsh 子进程日志打包成 zip，让用户另存

### M4.4 网络与磁盘错误

- [ ] 下载前检查磁盘可用空间 ≥ 200MB（`fs2` crate 或 `std::fs::statvfs` 封装）
- [ ] 无网络识别：`reqwest::Error::is_connect`、`is_timeout` → 提示"无法连接网络"
- [ ] 镜像源全失败：聚合每个源的错误信息，统一提示

### M4.5 系统托盘与窗口生命周期

原型中的托盘菜单定义信息架构：dsh 状态、显示主窗口、重启 dsh、检查更新、设置、关于、导出诊断日志和退出。实际实现使用 Tauri 2 的系统原生托盘菜单，不以 `TrayMenu.vue` 复刻 HTML/CSS 视觉，保证 macOS、Windows、Linux 的平台一致性与可访问性。

- [ ] `Cargo.toml` 启用 Tauri tray 所需 API；`lib.rs` 用 `TrayIconBuilder` 创建应用图标和原生菜单
- [ ] 菜单项与稳定 ID：禁用的状态行、`show-main-window`、`restart-dsh`、`check-dsh-update`、`open-settings`、`about`、`export-diagnostics`、分隔线、`quit`
- [ ] `on_menu_event` 按菜单 ID 分发：显示并聚焦主窗口；发出 `tray-open-settings` / `tray-check-dsh-update` / `tray-about` 前端事件；复用现有 host 生命周期执行重启；复用诊断导出命令；退出前 `shutdown_host()`
- [ ] `on_tray_icon_event`：左键点击显示并聚焦主窗口；右键及其他平台默认手势由原生菜单处理；不依赖仅 macOS 有效的手势
- [ ] 前端新增 `useTrayEvents.ts`，在 `App.vue` 订阅托盘事件，打开既有设置、升级或关于视图；组件不得直接调用 tray API
- [ ] Host 生命周期通过事件更新托盘状态行：启动中、运行中（携带当前 dsh 版本）和已停止/异常；状态行仅展示信息，不提供可点击操作
- [ ] 拦截 `tauri::RunEvent::ExitRequested` 与主窗口关闭：默认隐藏主窗口并保留托盘和 dsh；用户从 `quit` 明确退出时执行 host shutdown 后再退出进程
- [ ] 平台资源：补齐 `src-tauri/icons/` 的 tray 图标变体，macOS 模板图标保持单色，Windows/Linux 使用适配系统托盘的 16/32 px 图标

**验收**：

- 杀掉 dsh 子进程 3 次，观察崩溃弹窗
- 模拟 Node 下载文件被篡改（改 1 字节），SHA 校验失败
- 磁盘满时显示对应提示
- 启动后托盘显示“运行中”及当前 dsh 版本；重启 dsh 后状态依次变化为“启动中”与“运行中”
- 从托盘显示主窗口、打开设置、检查更新和导出诊断日志，确认分别触发对应既有流程
- 关闭主窗口后 dsh 持续运行；从托盘退出后 dsh 被关闭且应用进程退出

---

## 8. M5 — 发布

### M5.1 CI Matrix

- [ ] `.github/workflows/ci.yml`：
  - matrix：`macos-14 (arm64)`、`macos-13 (x64)`、`windows-latest (x64)`、`ubuntu-22.04 (x64)`
  - 步骤：`actions/setup-node`、`dtolnay/rust-toolchain`、`pnpm/action-setup`、`cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`、`pnpm install --frozen-lockfile`、`pnpm lint`、`pnpm tauri build`
- [ ] 缓存：`Swatinem/rust-cache` + pnpm store

### M5.2 打包配置

- [ ] `tauri.conf.json`：
  - macOS：`dmg`，`signingIdentity` 从 secret 注入
  - Windows：`nsis` + `msi`（M5 先 NSIS）
  - Linux：`appimage` + `deb`
- [ ] Bundle resources：`icon.icns` / `icon.ico` 来自原型配套资源

### M5.3 签名与公证（macOS）

- [ ] `APPLE_DEVELOPER_ID`、`APPLE_ID`、`APPLE_PASSWORD`、`APPLE_TEAM_ID` 注入 CI
- [ ] `tauri build` 触发 `codesign` + `xcrun notarytool submit`
- [ ] Hardened Runtime entitlements：允许执行用户目录二进制（`com.apple.security.cs.allow-unsigned-executable-memory` 视实际需要）
- [ ] 给下载的 node 二进制单独签名（启动时按需 `codesign --force --options runtime`）

### M5.4 壳子自动更新

- [ ] `tauri-plugin-updater` 集成
- [ ] `tauri.conf.json.plugins.updater`：pubkey + endpoints 指向 GitHub Releases `latest.json`
- [ ] `release.yml` 用 `tauri-action` 上传签名后的 bundle + `latest.json`
- [ ] 启动时后台检查壳子更新，**不**与 dsh 升级同窗口推

### M5.5 镜像源默认值与发布检查清单

- [ ] 发布前 checklist 文件 `docs/release-checklist.md`：
  - 确认 `DEFAULT_NODE_VERSION` 与 dsh `engines.node` 对齐
  - 确认 registry 中的 `latest` 可解析为完整 manifest
  - 确认所有镜像源可达
  - 确认 macOS 公证通过
- [ ] README 写明"首次启动必须联网"限制

**验收**：在干净 macOS / Windows / Linux 机器上安装冷启动，全流程跑通；CI 跑 matrix 全绿。

---

## 9. 测试策略

### 9.1 单元测试（Rust）

- `host/readiness.rs`：复刻 `host-supervisor.ts` 的边界用例（无 URL、冲突 URL、非 loopback、无端口、超长输出截断）
- `dsh/version.rs`：已发布版本排序、精确版本切换与回滚状态机
- `node/install.rs`：用 fixture tarball 验证解压、SHA 失败
- `state.rs`：并发写、损坏文件
- `mirror.rs`：URL 拼接、自定义源校验

覆盖率目标：核心模块 ≥ 90%。

### 9.2 集成测试（Rust）

- `tests/host_lifecycle.rs`：用 mock 子进程（脚本输出就绪行后 sleep）跑通 start/shutdown/超时/意外退出
- `tests/first_run.rs`：mock 镜像源服务器（`wiremock` crate）跑通下载流程

### 9.3 前端测试

- Vitest + `@vue/test-utils` + `@testing-library/vue`：状态机切换、错误展示
- Pinia store 用 `setActivePinia(createPinia())` 隔离测试
- shadcn-vue 组件交互测试：`Dialog` 打开/关闭、`Select` 选项触发、`Switch` 切换回写 store
- 关键交互（最新版本展示、安装确认、设置页）配快照测试

### 9.4 E2E

- `tauri-driver` + WebdriverIO
- 关键路径：首启 → 显示 `latest` 精确版本 → 下载 → 主界面 → 设置 → 确认更新最新版本
- 放行到 CI 的 nightly job

---

## 10. 任务拆解清单（按 PR 粒度）

> 每个 PR 独立可合并、独立可回滚；标题前缀 `[Mx]`。
> 标记说明：✅ 已完成 / 🚧 进行中 / ⬜ 未开始

### M1

- ✅ PR-001 [M1] 初始化 Tauri + Vue 工程、CI 骨架（详见 §M1.1）
- ✅ PR-002 [M1] `paths` + `state` + `error` + `logging`（详见 §M1.2）
- ✅ PR-003 [M1] `host/readiness` + `host/supervisor` + `host/lifecycle` + 单元测试（原计划的 PR-004 已并入本 PR，详见 §M1.3）
- ✅ PR-004 [M1] `commands` + Tauri 注册（原 PR-005，详见 §M1.4）
- ✅ PR-005 [M1] 前端骨架：Pinia store + 状态机 + MainView + ErrorDialog + vitest（原 PR-006，详见 §M1.5）

### M2

- ✅ PR-007 [M2] `mirror` 模块 + 探活（详见 §M2.1）
- ✅ PR-008 [M2] `node/download` + SHA 校验 + 进度事件（详见 §M2.2 + §M2.4）
- ✅ PR-009 [M2] `node/install` 跨平台解压 + 原子切换（详见 §M2.3）
- ✅ PR-010 [M2] `node/version` + engines 校验（已并入 PR-008，详见 §M2.4）
- ✅ PR-011 [M2] 前端首启向导 + 镜像源选择器（详见 §M2.5）

### M3

- ✅ PR-012 [M3] `dsh/registry` + 缓存（详见 §M3.1）
- ✅ PR-013 [M3] `dsh/install` + npm 封装 + `dsh/integrity`（详见 §M3.2）
- ✅ PR-014 [M3] `dsh/version`：指针切换 + 回滚 + 清理（详见 §M3.3）
- ✅ PR-014a [M3] 启动流程闭环修复（冒烟测试前置）（详见 §M3.3a）
- ⚠️ PR-015 [M3] 历史升级编排实现；由 PR-020c 删除并替换为显式版本安装
- ⚠️ PR-016 [M3] 历史升级设置与对话框；由 PR-020c 收敛为最新版本更新 UI
- ✅ PR-020b [M3] 首启 latest 展示 + 冻结安装计划（详见 §M3.3b）
- ⬜ PR-020c [M3] 显式版本安装与切换，移除后台升级策略（详见 §M3.4）

### M4

- ✅ PR-017 [M4] 崩溃计数 + 自动重启 + 弹窗（`host/crash.rs` 计数窗口策略 + supervisor 自动重启 + `host-crash-limit`/`host-restarted` 事件 + 前端 `CrashDialog.vue` + store 恢复 actions）
- ✅ PR-018 [M4] Node 升级基础设施；最新 dsh 更新的 Node 不兼容会阻止安装并保留当前版本
- ✅ PR-019 [M4] 错误文案 + 诊断导出（`LauncherError::user_message` 中文文案 + `export_diagnostics` zip 打包 state.json/壳子日志/dsh 日志 + 设置页导出按钮）
- ✅ PR-020 [M4] 磁盘/网络错误识别（`node/disk.rs` 200MB 下载前检查 + 下载错误文案区分网络/磁盘不足）
- ⬜ PR-020a [M4] 系统托盘与窗口生命周期（原生 tray 菜单 + 状态同步 + 前端事件桥接 + 退出时 host shutdown，详见 §M4.5）

### M5

- ⬜ PR-021 [M5] CI matrix + 缓存
- ⬜ PR-022 [M5] macOS 签名 + 公证
- ⬜ PR-023 [M5] Windows NSIS 打包
- ⬜ PR-024 [M5] Linux AppImage + deb
- ⬜ PR-025 [M5] 壳子自动更新 + `latest.json` 发布
- ⬜ PR-026 [M5] 发布检查清单 + README

---

## 11. 风险与对策

| 风险                    | 触发               | 对策                                                        |
| ----------------------- | ------------------ | ----------------------------------------------------------- |
| dsh 破坏性 CLI 变更     | dsh 改了就绪行格式 | `readiness.rs` 严格校验 + 失败回滚 + 写 Agent Note 记录契约 |
| Node 下载源全失败       | 区域性网络问题     | 多镜像源 + 用户自定义源 + 离线安装包作为后续开放问题        |
| macOS 沙盒限制执行 node | App Store 分发     | M5 走 Developer ID 非 App Store 路线                        |
| Windows 符号链接无权限  | 普通用户账户       | 已在设计中决定用 `current.json` 指针，PR-014 验证           |
| npm install 慢          | 160+ 依赖          | 进度条 + 日志 + 镜像源 + 后台预下载                         |
| 壳子升级与 dsh 升级冲突 | 同时触发           | 升级窗口互斥；壳子升级先完成再允许 dsh 升级                 |

---

## 12. 开放问题（待用户决策）

承接设计 §14，**不**在本计划内闭合：

1. 是否提供 "Offline" 安装包（预置 Node + dsh）？影响 M5 产物矩阵。
2. dsh web 前端资源与 dsh web 后端的兼容性是否需要壳子侧额外校验？
3. 多 profile 支持（stable + canary 并存）推迟到 v2，但 `state.json` schema 是否预留 `profiles` 字段？

建议在 M1 启动前对上述三项做一次性决策，避免 schema 后续迁移。

---

## 13. 实施前置条件

启动 M1 前必须就绪：

- [ ] macOS / Windows / Linux 三平台的本地开发环境
- [ ] Apple Developer ID 证书（M5 用，M1-M4 可空）
- [ ] GitHub repo + Actions 启用
- [ ] 与 dsh 团队确认 `@deepseek-ai/dsh` 包名与发布通道
- [ ] 与 dsh 团队确认 `lib/bin.js` 入口稳定性承诺
