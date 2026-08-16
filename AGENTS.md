# deepseek-harness-launcher

DeepSeek Harness (dsh) 的 Tauri 桌面壳子，负责托管 Node 运行时与 dsh 版本切换。

## 工程约定

- 仓内代码与注释风格：直接、具体、不叙述控制流。复述函数名/参数名/返回值的注释一律不写。
- 每个非平凡行为配测试；每个 PR 配 Agent Note。
- 命令、文件路径、配置项用 `code` 标注。
- 错误处理只在系统边界（用户输入、外部 API、子进程退出）做防御；内部代码互相信任类型契约。

## 关键文档

- [设计文档](./design/deepseek-harness-launcher-design.md)：架构、目录、契约、安全、跨平台
- [实施计划](./design/deepseek-harness-launcher-implementation-plan.md)：里程碑与 PR 拆解
- [原型 v4](./design/deepseek-harness-launcher-prototype.html)：UI 视觉、状态机、文案、shadcn token

## 技术栈

- Rust 1.80+ / edition 2021
- Tauri 2.x（`tauri`、`tauri-cli`、`tauri-plugin-dialog`、`tauri-plugin-opener`）
- Vue 3 + TypeScript 5（`<script setup>` 单文件组件）
- shadcn-vue（Radix Vue + Tailwind，组件源码落到 `src/components/ui/` 自管）
- Pinia / Vite 5 / Vitest
- pnpm

## 目录结构

```
deepseek-harness-launcher/
├── design/                # 设计与实施文档（只读）
├── src/                   # Vue 前端
│   ├── components/ui/     # shadcn-vue 生成的组件源码（自管）
│   ├── lib/               # 工具函数（cn、format、tauri invoke 封装）
│   ├── stores/            # Pinia
│   └── composables/       # Vue 组合式函数
└── src-tauri/
    └── src/
        ├── commands.rs    # #[tauri::command] 暴露面
        ├── state.rs       # AppState、state.json 读写
        ├── error.rs       # LauncherError + Serialize
        ├── paths.rs       # 数据/日志目录解析（跨平台）
        ├── mirror.rs      # 镜像源管理
        ├── logging.rs     # tracing + 滚动文件
        ├── node/          # Node 下载、解压、版本管理
        ├── dsh/           # dsh 注册表查询、安装、版本切换、完整性
        └── host/          # 子进程监管、就绪解析、生命周期
```

## 开发

```bash
pnpm install
pnpm tauri dev      # 起 dev webview
pnpm tauri build    # 打包
cargo test          # Rust 单测
pnpm lint           # ESLint
```

## 契约

壳子通过 spawn 子进程与 dsh 通信：

```
<node> --expose-internals <dsh-entry>/lib/bin.js web --host 127.0.0.1 --port 0
```

dsh 在 stdout 输出就绪行：`dsh web: http://127.0.0.1:<port>/`。约束见设计 §2.2。

## 错误与日志

- 壳子日志：`~/Library/Logs/deepseek-harness-launcher/app.log`（macOS）
- dsh 子进程日志：`<data_dir>/logs/dsh-<timestamp>.log`
- 所有用户可见错误必须给可操作的提示（见设计 §11.1）
