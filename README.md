# deepseek-harness-launcher

DeepSeek Harness（dsh）的 Tauri 桌面壳子。它在用户数据目录托管 Node 和 dsh，通过本地 loopback Web UI 运行 dsh。

## 当前状态

- 已实现：首启安装、Node 下载与 SHA-256 校验、dsh 安装、版本指针与回滚、崩溃恢复、托盘、错误提示、诊断导出和可选的 `dsh` 终端命令。
- 当前可通过 GitHub Actions 创建 arm64/x64 的 macOS 测试草稿包。它使用 ad-hoc 签名、未公证，仅限受控测试；正式公开分发仍需要 Apple Developer Program 的 Developer ID 签名与公证。
- CI 会在 macOS、Windows 和 Linux 上执行测试与基础打包验证。壳子自动更新、正式签名/公证和公开发布仍未启用。

首次启动需要联网下载 Node 和 dsh。当前 dsh 版本由 npm registry 在运行时解析，仓库不写死 `latest`。

## 开发

```bash
pnpm install
pnpm tauri dev
```

常用检查：

```bash
pnpm test
pnpm lint
pnpm build

cd src-tauri
cargo test --quiet
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

## 文档

- [设计文档](./design/deepseek-harness-launcher-design.md)
- [实施计划与当前待办](./design/deepseek-harness-launcher-implementation-plan.md)
- [测试计划](./design/deepseek-harness-launcher-test-plan.md)
- [原型](./design/deepseek-harness-launcher-prototype.html)

`design/page-flow-analysis.md` 是已归档的故障分析记录，不代表当前待办。
