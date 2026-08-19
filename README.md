# DeepSeek Harness Launcher

## 从打开应用，到开始工作

DeepSeek Harness Launcher 是 DeepSeek Harness 的桌面入口。它把运行环境、版本管理和本地启动流程集中处理，让你不必先配置 Node、dsh 或其他运行依赖，打开应用即可进入工作空间。

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./docs/image-black.png" />
    <source media="(prefers-color-scheme: light)" srcset="./docs/image.png" />
    <img src="./docs/image.png" alt="DeepSeek Harness Launcher 主界面" width="960" />
  </picture>
</p>

## 核心功能

### 自动准备运行环境

首次启动时，应用会自动下载匹配的 Node 运行时，并安装 DeepSeek Harness 所需的 dsh。所有运行文件由应用独立管理，不修改系统环境，也不会影响已有项目。

### 打开即进入 Harness

环境准备完成后，应用会自动启动本地服务，并在主窗口中打开 DeepSeek Harness。你可以直接创建会话、选择工作区和模式，开始处理任务。

### 版本升级由你掌控

发现 dsh 新版本时，应用只会进行提示，不会在工作过程中自动切换。确认升级后，应用才会完成下载、校验、切换和重启。

### 启动失败自动恢复

如果新版本启动失败，应用会自动回到上一个已验证可用的版本，减少升级对工作的影响。

## 安装与首次使用

1. 前往 [Releases](https://github.com/shetengteng/deepseek-harness-launcher/releases) 下载对应系统的安装包。
2. macOS 用户根据芯片选择版本：
   - Apple Silicon：`arm64` / `aarch64`
   - Intel：`x64`
3. macOS 打开 `.dmg`，将应用拖入「应用程序」；Windows 和 Linux 按对应安装包完成安装。
4. 首次启动时保持网络连接，应用会自动下载 Node 和 dsh，完成后即可进入 Harness。

### macOS 额外步骤

当前 macOS 安装包尚未经过 Apple 公证。首次打开时，系统可能提示「已损坏」或「无法验证开发者」。这是 macOS 对未公证应用的安全拦截，并不代表应用文件损坏。

将应用放入「应用程序」后，打开「终端」，执行：

```bash
xattr -cr /Applications/deepseek-harness-launcher.app
```

执行完成后重新打开应用。如果系统仍然拦截，请按住 Control 点击应用图标，选择「打开」。
