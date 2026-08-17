# deepseek-harness-launcher 测试计划

> 本文档描述当前代码库可执行的质量门禁，以及尚未覆盖的发布前测试。已完成 PR 的逐条历史用例以代码中的测试和 Git 历史为准；当前开发待办以 [实施计划](./deepseek-harness-launcher-implementation-plan.md) 的“当前待办”章节为准。

## 1. 通用门禁

每个涉及 Rust 或前端的 PR，按改动范围执行以下命令：

| 类别 | 命令 | 通过标准 |
| --- | --- | --- |
| Rust 测试 | `cargo test --quiet`（在 `src-tauri/`） | 全部通过 |
| Rust 格式 | `cargo fmt --all -- --check`（在 `src-tauri/`） | 无 diff |
| Rust 静态检查 | `cargo clippy --all-targets -- -D warnings`（在 `src-tauri/`） | 0 warning |
| 前端测试 | `pnpm test` | 全部通过 |
| 前端静态检查 | `pnpm lint` | 0 warning |
| 前端类型与构建 | `pnpm build` | 成功；脚本内已执行 `vue-tsc --noEmit` |

`pnpm typecheck` 不是仓库脚本，禁止将它作为独立门禁。

## 2. 当前覆盖范围

| 范围 | 已覆盖行为 |
| --- | --- |
| 状态与路径 | `state.json` 的读写、迁移、损坏处理和平台目录解析 |
| Node | 版本/`engines` 解析、镜像探活、下载、SHA-256、取消、解压和磁盘空间检查 |
| dsh | registry 元数据、安装完整性、版本指针、提升、回滚与清理 |
| Host | readiness 解析、子进程启动/关闭、输出缓冲和崩溃计数策略 |
| 前端 | 首启双任务视图、启动遮罩、设置页、错误展示与托盘事件桥接 |

2026-08-17 的本地基线：`cargo test --quiet` 通过 160 个 crate 测试和 4 个集成测试；`pnpm test` 通过 27 个测试；`pnpm lint && pnpm build` 通过。`cargo clippy --all-targets -- -D warnings` 仍由既有的 3 处 `too_many_arguments` 与 1 处 `trim_split_whitespace` 阻塞，未在本轮 P0 范围内修改。

## 3. 当前测试待办

### P0：更新与诊断

- [x] 首启界面挂载后自动将当前 latest 写入 `bootstrap_plan` 并开始安装；之后 registry 变化或重试均不得漂移该计划。
- [x] 更新必须安装通知或设置页已展示的精确版本；用户点击更新后立即提升 current 并重启，启动失败恢复 `known_good`。
- [x] 更新时 Node 不兼容、安装失败、取消和同版本重试都必须保留当前可用 dsh。
- [x] 日常启动时 current 的 Node/dsh 缺失或启动失败，应仅回退 `known_good` 一次；两者都不可用时进入修复流程。
- [x] dsh stdout/stderr 应写入独立文件；诊断导出只包含最近 3 份 dsh 日志，并有归档内容测试。
- [x] Webview 仅允许 launcher origin 与当前 dsh 的精确 origin；旧 Host、其他 loopback 端口和外部 URL 均被拒绝。用户点击来自当前 dsh iframe 的 `http/https` 外链，经来源与 payload 校验后使用系统浏览器打开。
- [x] dsh 不具有 Tauri `remote` capability；iframe 委派摄像头、麦克风、定位等浏览器能力并保留弹窗，由 dsh 与用户决定。launcher 不持久化或配置 dsh 项目目录，Host cwd 固定为托管 dsh 版本目录。

### P1：运行时与托盘

- [ ] 用 mock 子进程覆盖 Host 的就绪、超时、意外退出、自动重启、显式关闭和回滚。
- [ ] 覆盖确认 Node 升级、取消升级和原子切换后的 dsh 更新。
- [ ] 覆盖托盘状态在 Host 启动、运行、停止和异常时的更新；验证关闭主窗口后 Host 继续运行、托盘退出后 Host 结束。

### P2：端到端与发布

- [ ] 建立 Tauri E2E：首启、更新、立即重启、回滚与崩溃恢复。
- [ ] 在 CI 中运行单元/集成测试、lint、构建；E2E 可作为 nightly 门禁。
- [ ] 在干净 macOS、Windows、Linux 环境执行冷启动和受管 Node/dsh 安装冒烟测试。

## 4. 发布验收

进入公开发布前，除通用门禁外必须确认：

- [ ] 经过签名/公证的 macOS 包可启动受管 Node 和 dsh。
- [ ] 安装包不含用户数据、Node 运行时或 dsh 版本目录。
- [ ] 首启断网、镜像失败、校验失败、磁盘空间不足与 dsh 启动超时均显示可操作提示。
- [ ] 诊断压缩包不包含超出 `state.json` 与日志范围的用户工作区文件。
- [ ] 发布检查清单、README 和 CI 的命令与本文件保持一致。
