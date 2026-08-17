# deepseek-harness-launcher

DeepSeek Harness（dsh）的 Tauri 桌面壳子。它在用户数据目录托管 Node 和 dsh，通过本地 loopback Web UI 运行 dsh。

## 当前状态

- 已实现：首启安装、Node 下载与 SHA-256 校验、dsh 安装、版本指针与回滚、崩溃恢复、托盘、错误提示和诊断导出。
- 尚未达到公开发布标准：更新必须改为“展示版本后显式确认、`pending` 安装和确认重启”；dsh 子进程文件日志、CI、签名/公证和跨平台安装包仍在计划中。

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
