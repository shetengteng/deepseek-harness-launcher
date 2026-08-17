# dsh 命令行入口设计

## 背景

启动器把 Node.js 与 `@deepseek-ai/dsh` 安装在应用数据目录，并以绝对路径启动 Web profile。该路径没有加入用户 shell 的 `PATH`，因此 App 已运行时，在 Terminal 输入 `dsh` 仍会得到“command not found”。GUI 进程也不能修改其父 shell 或已经打开的 Terminal 的环境变量。

## 目标

- 用户在设置中主动安装一次后，可在新打开的终端直接运行 `dsh`。
- `dsh plugin --profile web add …` 与启动器启动的 Web 服务读取同一个 `DSH_HOME`。
- dsh 或托管 Node.js 更新后，命令无需重装。
- 不覆盖已有的非启动器 `dsh` 命令，不修改 shell 配置文件。

## 非目标

- 不把 `dsh` 或 Node.js 全局安装到 npm 的全局目录。
- 不修改当前或已运行 Terminal 的 `PATH`。
- 不让 CLI 依赖启动器正在运行；CLI 应可独立启动 dsh，随后 GUI 使用同一 profile。

## 方案

设置页提供“安装 `dsh` 命令”操作。它在 `~/.local/bin/dsh`（Windows 为 `%USERPROFILE%\\.local\\bin\\dsh.cmd`）写入一个稳定的 shim。安装结果包含命令路径与将该目录加入 `PATH` 的一次性说明；用户关闭并重新打开 Terminal 后生效。

shim 不记录具体版本：每次运行时读取启动器数据目录的 Node `VERSION`、dsh `current` 指针，以及对应的 `node_modules/@deepseek-ai/dsh/lib/bin.js`。之后执行：

```text
<managed-node> --expose-internals <current-dsh-entry> <user arguments>
```

使用当前指针使 dsh 回滚和升级自动生效。启动器和 shim 都继承 `DSH_HOME`；未设置时 dsh 默认使用 `$HOME/.dsh`，所以 profile 与插件目录保持一致。

`dsh plugin` 会在 profile 目录直接调用 `pnpm`。启动器在数据目录的 `bin/` 下同时生成一个 `pnpm` wrapper；它通过托管 Node 自带的 Corepack 执行 `pnpm`。dsh shim 和 GUI Host 启动时均会把该目录置于 `PATH` 首位，因此不要求用户单独全局安装 pnpm。

## 安全与错误处理

- 如果 dsh 或 Node 尚未安装，拒绝创建 shim 并提示完成首次启动向导。
- 写入前检查目标：已存在且内容不是启动器 shim 时拒绝覆盖。
- 文件用临时文件写入后原子替换；Unix shim 设置为可执行。
- shim 在运行时验证 Node 和 dsh 入口，损坏或已卸载时输出可操作错误并退出非零。
- 用户目录路径会按目标 shell 的规则转义，避免路径中的空格或引号改变命令含义。

## 测试

- 覆盖 Unix shim 的版本解析、参数透传、Corepack/pnpm 调用、路径转义与缺失运行时错误。
- 覆盖拒绝覆盖非启动器文件、允许更新已有 shim 和可执行权限。
- 覆盖 Tauri 命令返回的路径与 PATH 提示。
- 前端组件测试按钮状态、成功提示和错误展示。
