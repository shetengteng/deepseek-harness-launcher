# deepseek-harness-launcher 测试设计

> 配套 [deepseek-harness-launcher-implementation-plan.md](./deepseek-harness-launcher-implementation-plan.md) §9 测试策略，按 PR 粒度列出每个环节的具体测试用例与验收门禁。
> 每个 PR 合入前必须跑通本文件列出的全部测试；CI 负责回归。

## 0. 通用门禁（所有 PR 都跑）

| 类别           | 命令                                        | 通过标准  |
| -------------- | ------------------------------------------- | --------- |
| Rust 单元/集成 | `cargo test --no-fail-fast`                 | 全部 pass |
| Rust 风格      | `cargo clippy --all-targets -- -D warnings` | 0 warning |
| Rust 格式      | `cargo fmt --all -- --check`                | 无 diff   |
| 前端单测       | `pnpm test`                                 | 全部 pass |
| 前端类型       | `pnpm typecheck`                            | 0 error   |
| 前端构建       | `pnpm build`                                | 成功      |

覆盖率目标：Rust 核心模块（`state` / `host` / `node` / `dsh`）行覆盖 ≥ 90%；前端 store 与状态机 ≥ 85%。

---

## M1 — 最小可用

### PR-001 工程脚手架

**测试目标**：Tauri 工程能编译、能启动、shadcn-vue 接入正确。

| 用例                           | 类型 | 期望                                                                |
| ------------------------------ | ---- | ------------------------------------------------------------------- |
| `cargo check --lib`            | 编译 | 0 error                                                             |
| `pnpm tauri dev` 启动 webview  | 手动 | 窗口打开，Vite 1420 端口 + Rust 二进制正常运行                      |
| shadcn-vue 组件渲染（Button）  | 手动 | webview 显示一个按钮                                                |
| `tsconfig.json` 无递归类型错误 | 编译 | `pnpm typecheck` 0 error（已知 `use-toast.ts:82` 遗留 TS2589 除外） |

### PR-002 状态层（state + paths + error + logging）

**测试目标**：`state.json` 原子读写、跨平台路径解析、错误可被前端序列化、日志按天滚动。

| 文件         | 用例                                                           | 期望                                                                            |
| ------------ | -------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `state.rs`   | `load_from` 缺失文件 → `FirstRun`                              | 不写默认空文件                                                                  |
| `state.rs`   | `load_from` 损坏 JSON → `StateCorrupt`                         | 错误携带 path + cause                                                           |
| `state.rs`   | `load_from` schema_version 不匹配 → `UnsupportedSchemaVersion` | 返回实际版本号                                                                  |
| `state.rs`   | `save_to` 写入后 `load_from` 读回                              | 字段相等                                                                        |
| `state.rs`   | 8 线程并发 `save_to` 同一路径                                  | 至少一个成功；最终文件是有效 JSON                                               |
| `state.rs`   | `save_to` 写入被中途打断（模拟 `.tmp` 残留）                   | 主文件不受影响                                                                  |
| `state.rs`   | 缺失字段（`#[serde(default)]`）                                | 用默认值回退，不报错                                                            |
| `paths.rs`   | macOS 数据目录                                                 | `~/Library/Application Support/io.deepseek/DeepSeek/deepseek-harness-launcher/` |
| `paths.rs`   | macOS 日志目录                                                 | `~/Library/Logs/deepseek-harness-launcher/`                                     |
| `paths.rs`   | 非 macOS 日志目录                                              | `<data_dir>/logs/`                                                              |
| `paths.rs`   | `ensure_dirs` 在不存在时创建                                   | 目录递归创建成功                                                                |
| `error.rs`   | `LauncherError::StateCorrupt` 序列化                           | `{ kind, message, data }` 三段式                                                |
| `error.rs`   | 所有 variant 的 `kind_str`                                     | 与设计 §11.1 对齐                                                               |
| `logging.rs` | `init` 后 `tracing::info!`                                     | 文件 `app.YYYY-MM-DD` 出现在 `log_dir`                                          |
| `logging.rs` | debug 构建输出到 stderr                                        | release 不输出（避免污染 GUI stdout）                                           |

### PR-003 Host 监管（readiness + supervisor + lifecycle）

**测试目标**：就绪行解析正确、子进程生命周期可控、环境变量隔离。

| 文件            | 用例                                                                       | 期望                            |
| --------------- | -------------------------------------------------------------------------- | ------------------------------- |
| `readiness.rs`  | `dsh web: http://127.0.0.1:51329/\n`                                       | 返回 `http://127.0.0.1:51329`   |
| `readiness.rs`  | split chunks（`dsh web: http://127.` + `0.0.1:42` + `/\n`）                | 拼接后正确解析                  |
| `readiness.rs`  | 非 readiness 行                                                            | `None`，不影响状态              |
| `readiness.rs`  | 非 loopback（`example.com`）                                               | `InvalidUrl`                    |
| `readiness.rs`  | https scheme                                                               | `InvalidUrl`                    |
| `readiness.rs`  | 端口 0                                                                     | `InvalidUrl`                    |
| `readiness.rs`  | query string                                                               | `InvalidUrl`                    |
| `readiness.rs`  | hash                                                                       | `InvalidUrl`                    |
| `readiness.rs`  | 缺失端口                                                                   | `InvalidUrl`                    |
| `readiness.rs`  | 冲突 URL（先 8080 后 9000）                                                | `Conflicting`                   |
| `readiness.rs`  | 重复同 URL                                                                 | 幂等，不报错                    |
| `readiness.rs`  | finalize 无 readiness                                                      | `NoReadiness`                   |
| `readiness.rs`  | finalize 解析 pending tail（无 trailing newline）                          | 正确解析                        |
| `readiness.rs`  | `\r\n` 结尾                                                                | 去掉 `\r` 后解析                |
| `lifecycle.rs`  | `is_passthrough("PATH")` 等                                                | PATH/HOME/USERPROFILE/LANG 通过 |
| `lifecycle.rs`  | `is_passthrough("DSH_*")`                                                  | 通过                            |
| `lifecycle.rs`  | `is_passthrough("LC_*")`                                                   | 通过                            |
| `lifecycle.rs`  | `is_passthrough("RUST_*")`                                                 | 拒绝                            |
| `lifecycle.rs`  | `is_passthrough("TAURI_*")`                                                | 拒绝                            |
| `lifecycle.rs`  | `is_passthrough("npm_*")`/`npm_config_*`/`npm_lifecycle_*`/`npm_package_*` | 拒绝                            |
| `lifecycle.rs`  | `filtered_env` 端到端                                                      | 上述被拒键不在结果中            |
| `lifecycle.rs`  | `cli_entry_from_env` 未设置                                                | `PathResolve` 错误              |
| `lifecycle.rs`  | `cli_entry_from_env` 已设置                                                | 返回 `PathBuf`                  |
| `supervisor.rs` | `shutdown` 未 start                                                        | no-op，不 panic                 |
| `supervisor.rs` | `start` 在 `shutdown` 后                                                   | `AlreadyShutdown`               |
| `supervisor.rs` | `append_output` 超过 32KB                                                  | 头部截断到 32KB                 |
| `supervisor.rs` | `append_output` 中文非字符边界                                             | 截断到最近字符边界              |

**集成测试（推迟到 PR-004 命令层后）**：

- `tests/host_lifecycle.rs`：mock 子进程（shell 脚本输出 readiness 行后 sleep）跑通 start → shutdown
- `tests/host_lifecycle.rs`：mock 子进程不发 readiness 行，验证 90s 超时（用短超时配置覆盖）

### PR-004 Tauri 命令层

**测试目标**：`#[tauri::command]` 正确注册、错误能被前端解析、Host 生命周期通过命令可控。

| 文件          | 用例                          | 期望                                                       |
| ------------- | ----------------------------- | ---------------------------------------------------------- |
| `commands.rs` | `launcher_status` 首启        | `StatusSnapshot { phase: "first_run", host_origin: None }` |
| `commands.rs` | `launcher_status` Host ready  | `phase: "ready"`, `host_origin: Some(origin)`              |
| `commands.rs` | `start_host` 成功             | 返回 origin 字符串                                         |
| `commands.rs` | `start_host` 在 shutdown 后   | 返回 `AlreadyShutdown` 序列化错误                          |
| `commands.rs` | `shutdown_host` 幂等          | 多次调用都返回 `Ok(())`                                    |
| `commands.rs` | 错误序列化                    | `{ kind, message, data }` 能被前端 TS 类型解析             |
| `lib.rs`      | `tauri::Builder` 注册所有命令 | `invoke_handler` 包含全部 `#[tauri::command]`              |
| `lib.rs`      | `manage(AppState::shared())`  | state 注入到 Tauri 状态                                    |

**集成测试**：

- `tests/commands.rs`：用 `tauri::test::mock_app` 调用命令，验证返回结构

### PR-005 前端骨架

**测试目标**：Pinia store 状态机正确、shadcn-vue 组件交互、错误展示。

| 文件                         | 用例                   | 期望                                              |
| ---------------------------- | ---------------------- | ------------------------------------------------- |
| `stores/launcher.ts`         | 初始状态               | `phase: 'loading'`, `origin: null`, `error: null` |
| `stores/launcher.ts`         | `fetchStatus` 首启响应 | `phase: 'first_run'`                              |
| `stores/launcher.ts`         | `startHost` 成功       | `phase: 'ready'`, `origin` 非空                   |
| `stores/launcher.ts`         | `startHost` 失败       | `phase: 'error'`, `error` 含 kind/message         |
| `stores/launcher.ts`         | `shutdownHost` 后      | `phase: 'idle'`, `origin: null`                   |
| `stores/launcher.ts`         | `resetError`           | `error: null`，phase 回到 idle                    |
| `components/MainView.vue`    | phase=loading          | 渲染 Loading 组件                                 |
| `components/MainView.vue`    | phase=first_run        | 渲染 FirstRun 入口                                |
| `components/MainView.vue`    | phase=ready            | 渲染 iframe/webview 指向 origin                   |
| `components/MainView.vue`    | phase=error            | 渲染 ErrorDialog                                  |
| `components/ErrorDialog.vue` | 关闭按钮               | 触发 `resetError`                                 |
| `App.vue`                    | 挂载时调 `fetchStatus` | 调用一次                                          |

**测试工具**：Vitest + `@vue/test-utils` + `setActivePinia(createPinia())`。

---

## M2 — Node 托管

### PR-006 镜像源（mirror.rs）

**测试目标**：URL 拼接正确、自定义源校验、探活超时。

| 文件        | 用例                       | 期望           |
| ----------- | -------------------------- | -------------- |
| `mirror.rs` | 默认源（npmmirror）        | URL 模板正确   |
| `mirror.rs` | 自定义源（带/不带尾斜杠）  | 拼接后路径唯一 |
| `mirror.rs` | 自定义源非 https           | 拒绝           |
| `mirror.rs` | 自定义源非 `https://` 前缀 | 拒绝           |
| `mirror.rs` | `probe` 200 响应           | `Ok(())`       |
| `mirror.rs` | `probe` 404                | `Err`          |
| `mirror.rs` | `probe` 超时（< 1s 配置）  | `Err(Timeout)` |
| `mirror.rs` | `probe` 连接拒绝           | `Err`          |

**集成测试**：`tests/mirror.rs` 用 `wiremock` 起 mock server。

### PR-007 下载与校验（node/download.rs）

**测试目标**：下载流式写入、SHA 校验、进度事件。

| 文件          | 用例                      | 期望                              |
| ------------- | ------------------------- | --------------------------------- |
| `download.rs` | 下载成功                  | 文件落盘，大小匹配 Content-Length |
| `download.rs` | SHA256 匹配               | `Ok(downloaded_path)`             |
| `download.rs` | SHA256 不匹配             | `Err(ChecksumMismatch)`，删除文件 |
| `download.rs` | 下载中断（mock 关闭连接） | `Err(Io)`，部分文件清理           |
| `download.rs` | 进度事件                  | 每 64KB 触发一次 `ProgressEvent`  |
| `download.rs` | 404                       | `Err(NotFound)`                   |
| `download.rs` | 5xx 重试                  | 重试 3 次后 `Err`                 |

**集成测试**：`tests/download.rs` 用 `wiremock` 返回固定字节的 tarball fixture。

### PR-008 解压与安装（node/install.rs）

**测试目标**：跨平台解压、原子切换、残留清理。

| 文件         | 用例                      | 期望                          |
| ------------ | ------------------------- | ----------------------------- |
| `install.rs` | macOS `.tar.gz` 解压      | `node` 二进制在目标目录可执行 |
| `install.rs` | Windows `.zip` 解压       | `node.exe` 在目标目录可执行   |
| `install.rs` | Linux `.tar.xz` 解压      | `node` 二进制在目标目录可执行 |
| `install.rs` | 解压到临时目录后 `rename` | 原子切换到最终位置            |
| `install.rs` | 目标目录已有旧版本        | 旧版本被替换                  |
| `install.rs` | 解压中断（磁盘满模拟）    | 临时目录清理，旧版本保留      |
| `install.rs` | `node --version` 子进程   | 返回下载的版本号              |

**集成测试**：`tests/install.rs` 用 fixture tarball（小体积真实 Node 二进制）。

### PR-009 版本与 engines（node/version.rs）

**测试目标**：semver 解析与 `engines.node` 校验。

| 文件         | 用例                                      | 期望                        |
| ------------ | ----------------------------------------- | --------------------------- |
| `version.rs` | semver 解析 `v20.18.0`                    | `Version(20, 18, 0)`        |
| `version.rs` | semver range `>=20.18.0` 匹配 `20.19.0`   | true                        |
| `version.rs` | semver range `>=20.18.0` 不匹配 `20.17.0` | false                       |
| `version.rs` | engines 要求 `>=20.18` 当前 `20.18.0`     | 通过                        |
| `version.rs` | engines 要求 `>=20.18` 当前 `20.17.0`     | `Err(EngineMismatch)`       |
| `version.rs` | 选择最新 LTS                              | 返回 LTS 标记的最新版本     |
| `version.rs` | 选择最新 current                          | 返回 current 标记的最新版本 |

### PR-010 首启向导（FirstRun.vue）

**测试目标**：下载流程可视、镜像源切换、进度展示。

| 文件           | 用例       | 期望                           |
| -------------- | ---------- | ------------------------------ |
| `FirstRun.vue` | 挂载       | 显示镜像源选择器 + 下载按钮    |
| `FirstRun.vue` | 选择镜像源 | store 更新 `mirror_url`        |
| `FirstRun.vue` | 点击下载   | 触发 `startDownload` 命令      |
| `FirstRun.vue` | 下载中     | 进度条显示百分比，禁用按钮     |
| `FirstRun.vue` | 下载失败   | 显示错误，允许重试             |
| `FirstRun.vue` | 下载成功   | 触发 `fetchStatus`，进入 ready |

---

## M3 — dsh 托管

### PR-011 Registry 查询（dsh/registry.rs）

**测试目标**：npm registry 查询、版本列表解析、缓存。

| 文件          | 用例                    | 期望              |
| ------------- | ----------------------- | ----------------- |
| `registry.rs` | 查询 `@deepseek-ai/dsh` | 返回版本列表      |
| `registry.rs` | 缓存命中                | 不发网络请求      |
| `registry.rs` | 缓存过期（TTL 5min）    | 重新查询          |
| `registry.rs` | dist-tag `latest`       | 返回最新版本      |
| `registry.rs` | tarball URL             | 拼接正确          |
| `registry.rs` | 网络错误                | `Err`，不污染缓存 |

**集成测试**：`tests/registry.rs` 用 `wiremock` 返回 fixture JSON。

### PR-012 安装（dsh/install.rs）

**测试目标**：npm install 封装、完整性校验、原子切换。

| 文件         | 用例                  | 期望                                 |
| ------------ | --------------------- | ------------------------------------ |
| `install.rs` | 安装指定版本          | `node_modules/@deepseek-ai/dsh` 存在 |
| `install.rs` | `cli_entry` 解析      | 指向 `lib/bin.js`                    |
| `install.rs` | 完整性校验（SHASUMS） | 通过                                 |
| `install.rs` | 完整性校验失败        | `Err(Integrity)`，回滚               |
| `install.rs` | 原子切换（symlink）   | 旧 `current` 保留为 `previous`       |
| `install.rs` | 安装中断              | 临时目录清理                         |

### PR-013 版本切换与回滚（dsh/version.rs）

**测试目标**：指针切换、known_good 回滚、清理。

| 文件         | 用例                      | 期望                                      |
| ------------ | ------------------------- | ----------------------------------------- |
| `version.rs` | 切换到新版本              | `current` 指向新版本                      |
| `version.rs` | 回滚到 `known_good`       | `current` 切回，`bad` 标记失败版本        |
| `version.rs` | 清理旧版本（保留 N 个）   | 超出 N 个的版本被删除                     |
| `version.rs` | `known_good` 未设置时回滚 | `Err(NoKnownGood)`                        |
| `version.rs` | 版本状态机                | installed → active → known_good → retired |

### PR-014 / PR-015 历史自动升级流程

这两项记录原先的范围、轮询和候选版本升级设计，已由 PR-020c 删除，不再新增或维护对应测试。

### PR-020c 最新版本手动更新

**测试目标**：启动时轻量检查 registry；发现新版后从右侧非阻塞提示告知用户，只有用户明确确认后才安装；任何安装失败都保留当前版本。

| 文件           | 用例                     | 期望                                          |
| -------------- | ------------------------ | --------------------------------------------- |
| `commands.rs`  | `latest` 有完整 manifest | 返回精确版本                                  |
| `commands.rs`  | `latest` 缺失 manifest   | 返回 registry 错误，不安装                    |
| `install.rs`   | 安装失败                 | 清理 staging 目录，不改 `current`             |
| `version.rs`   | pending 版本启动失败     | 回滚到 `known_good` 并标记失败版本为 `broken` |
| `App.vue`      | 启动检查失败             | 不阻塞 dsh 启动，不弹错误窗口                  |
| `UpdateNotice` | latest 不同于 current    | 右侧显示非阻塞提示与“更新”按钮                |
| `UpdateNotice` | 用户关闭提示             | 不下载、不重启，继续使用当前版本              |
| `UpdateNotice` | 同一版本再次启动         | 不重复弹出，设置页仍可手动检查                  |
| `Settings.vue` | 显示当前 latest          | 显示精确版本与“检查更新”按钮                  |
| `Settings.vue` | current 等于 latest      | 禁用“更新到最新版本”按钮                      |
| `Settings.vue` | 明确点击更新             | 使用已展示的精确版本调用安装                  |
| `install.rs`   | 安装成功                 | 设置 `pending`，不立即重启                    |
| `install.rs`   | 安装失败                 | 清理目标目录，保留当前版本并显示可操作错误    |
| `UpdateNotice` | 用户选择重试             | 重试同一精确版本，不重新漂移到新的 `latest`   |
| `UpdateNotice` | 用户更换源重试           | 使用新源重试同一精确版本                      |

---

## M4 — 健壮性

### PR-016 崩溃恢复

**测试目标**：崩溃计数、自动重启、弹窗。

| 文件           | 用例                        | 期望                     |
| -------------- | --------------------------- | ------------------------ |
| `lifecycle.rs` | Host 正常退出（exit 0）     | 不计数，不重启           |
| `lifecycle.rs` | Host 崩溃（exit 非 0）      | `crash_counter += 1`     |
| `lifecycle.rs` | 连续崩溃 < 3 次             | 自动重启                 |
| `lifecycle.rs` | 连续崩溃 ≥ 3 次             | 不重启，弹错误对话框     |
| `lifecycle.rs` | 成功运行 5 分钟后崩溃       | `crash_counter` 重置为 0 |
| `lifecycle.rs` | `state.json` 持久化崩溃计数 | 重启壳子后计数保留       |

**集成测试**：`tests/crash_recovery.rs` 用 mock 子进程模拟崩溃。

### PR-017 Node 升级流程

**测试目标**：Node 版本升级、回滚、 engines 校验。

| 文件              | 用例                     | 期望                  |
| ----------------- | ------------------------ | --------------------- |
| `node/upgrade.rs` | 检测新 Node 版本         | 返回新版本号          |
| `node/upgrade.rs` | 下载 + 安装新 Node       | 成功                  |
| `node/upgrade.rs` | 新 Node 不满足 engines   | `Err(EngineMismatch)` |
| `node/upgrade.rs` | 切换后 Host 崩溃         | 回滚到旧 Node         |
| `node/upgrade.rs` | 回滚后 `known_good` 更新 | 指向旧 Node           |

### PR-018 错误提示与日志

**测试目标**：错误文案、诊断导出。

| 文件                | 用例                    | 期望                                      |
| ------------------- | ----------------------- | ----------------------------------------- |
| `error_messages.rs` | `ChecksumMismatch` 文案 | 包含期望/实际 SHA + 下载 URL              |
| `error_messages.rs` | `EngineMismatch` 文案   | 包含 engines 要求 + 当前版本              |
| `error_messages.rs` | `Integrity` 文案        | 包含文件路径 + 期望 SHASUMS               |
| `diagnostics.rs`    | 导出诊断 zip            | 包含 `app.YYYY-MM-DD` 日志 + `state.json` |
| `diagnostics.rs`    | 导出脱敏                | 不包含 API key、用户凭证                  |

### PR-019 网络与磁盘错误

**测试目标**：错误识别、重试、用户提示。

| 文件        | 用例         | 期望                    |
| ----------- | ------------ | ----------------------- |
| `errors.rs` | DNS 解析失败 | `Err(Dns)`              |
| `errors.rs` | 连接超时     | `Err(Timeout)`          |
| `errors.rs` | 磁盘满       | `Err(DiskFull)`         |
| `errors.rs` | 权限不足     | `Err(PermissionDenied)` |
| `errors.rs` | 磁盘满重试   | 提示用户清理后重试      |

---

## M5 — 发布

### PR-020 CI Matrix

**测试目标**：CI 跨平台构建、测试。

| 平台           | 用例                                      | 期望              |
| -------------- | ----------------------------------------- | ----------------- |
| macOS-latest   | `cargo test` + `pnpm test` + `pnpm build` | 全绿              |
| windows-latest | 同上                                      | 全绿              |
| ubuntu-latest  | 同上                                      | 全绿              |
| 缓存命中       | `cargo` + `pnpm` 缓存                     | 第二次构建 < 1min |

### PR-021 打包配置

**测试目标**：跨平台打包产物正确。

| 平台    | 产物                 | 验证                                        |
| ------- | -------------------- | ------------------------------------------- |
| macOS   | `.dmg` + `.app`      | 双击安装，能启动                            |
| Windows | `.exe` (NSIS)        | 双击安装，能启动                            |
| Linux   | `.AppImage` + `.deb` | `./deepseek-harness-launcher.AppImage` 启动 |

### PR-022 签名与公证（macOS）

**测试目标**：签名有效、公证通过。

| 用例                                | 期望               |
| ----------------------------------- | ------------------ |
| `codesign --verify --deep --strict` | 通过               |
| `spctl --assess --type execute`     | 通过               |
| `xcrun notarytool submit`           | 状态 `Accepted`    |
| Gatekeeper 双击启动                 | 不弹"未识别开发者" |

### PR-023 壳子自动更新

**测试目标**：Tauri updater 配置、签名校验。

| 用例                           | 期望                        |
| ------------------------------ | --------------------------- |
| `tauri.conf.json` updater 配置 | endpoints 指向 release JSON |
| 签名公钥配置                   | `pubkey` 存在               |
| 下载更新包                     | 校验签名通过                |
| 签名失败                       | 拒绝安装                    |
| 安装更新                       | 旧版本替换为新版本          |

### PR-024 镜像源默认值与发布检查清单

**测试目标**：默认镜像源、发布前检查。

| 用例               | 期望                              |
| ------------------ | --------------------------------- |
| 默认镜像源（国内） | `https://registry.npmmirror.com/` |
| 默认镜像源（海外） | `https://registry.npmjs.org/`     |
| 地区检测           | 根据系统 locale 选择默认          |
| 发布检查清单       | 所有项勾选                        |

---

## 测试工具清单

| 工具                         | 用途                  | 版本   |
| ---------------------------- | --------------------- | ------ |
| `cargo test`                 | Rust 单元/集成测试    | 1.80+  |
| `cargo clippy`               | Rust 风格检查         | 1.80+  |
| `cargo tarpaulin`            | Rust 覆盖率           | latest |
| `vitest`                     | 前端单测              | 1.x    |
| `@vue/test-utils`            | Vue 组件测试          | 2.x    |
| `@testing-library/vue`       | Vue 组件交互测试      | 8.x    |
| `wiremock`                   | Rust mock HTTP server | 0.6    |
| `tauri-driver` + WebdriverIO | E2E                   | latest |

## 测试文件布局

```
src-tauri/
  src/
    host/readiness.rs          # 内嵌 #[cfg(test)] mod tests
    host/lifecycle.rs          # 内嵌
    host/supervisor.rs         # 内嵌
    state.rs                   # 内嵌
  tests/
    host_lifecycle.rs          # 集成：mock 子进程
    mirror.rs                  # 集成：wiremock
    download.rs                # 集成：wiremock + fixture
    install.rs                  # 集成：fixture tarball
    registry.rs                # 集成：wiremock
    upgrade.rs                 # 集成：mock registry + tarball
    crash_recovery.rs          # 集成：mock 崩溃子进程
    commands.rs                # 集成：tauri mock_app
  fixtures/
    node-v20.18.0-darwin-arm64.tar.gz
    node-v20.18.0-linux-x64.tar.xz
    node-v20.18.0-win-x64.zip
    dsh-0.1.0.tgz
    SHASUMS256.txt

src/
  stores/__tests__/launcher.test.ts
  components/__tests__/MainView.test.ts
  components/__tests__/FirstRun.test.ts
  components/__tests__/UpgradeDialog.test.ts
  components/__tests__/SettingsPage.test.ts
  components/__tests__/ErrorDialog.test.ts

e2e/
  first-run.spec.ts            # 首启 → 下载 → 主界面
  upgrade.spec.ts              # 触发升级 → 成功
  crash-recovery.spec.ts       # 模拟崩溃 → 自动重启
```

## 验收门禁汇总

每个 PR 合入前必须满足：

1. 本文件对应章节的全部用例通过
2. `cargo test --no-fail-fast` 全绿
3. `cargo clippy --all-targets -- -D warnings` 全绿
4. `pnpm test` 全绿（前端 PR）
5. `pnpm typecheck` 0 error（前端 PR）
6. 实施计划 md 中对应章节勾选 `[x]`
7. PR 描述列出已测试的用例清单
