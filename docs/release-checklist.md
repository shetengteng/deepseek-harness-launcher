# 发布检查清单

此清单适用于 GitHub Actions 的 `Draft unsigned test release` 工作流。它构建 macOS arm64/x64、Windows x64 和 Linux x64 测试包：macOS 使用 ad-hoc 签名，Windows/Linux 不签名，不需要 Apple Developer 证书，也不会公证。

测试包仅供受控测试，必须保持 GitHub 草稿 Release 和 prerelease 状态，不能作为公开分发版本。不要为了安装测试包而关闭系统级 Gatekeeper 或 SmartScreen。

## 构建测试发布

- 在 Actions 手动运行 `Draft unsigned test release`，并在确认输入框填写 `unsigned-test`。
- 发布成功后只保留 GitHub Draft + Prerelease 状态，确认 Release 说明中明确写有 unsigned/ad-hoc test。

## macOS 测试包

- 下载对应架构的 `.dmg`：Apple Silicon 选 arm64，Intel 选 x64。
- 把 `deepseek-harness-launcher.app` 拖到「应用程序」后执行：

```bash
xattr -cr /Applications/deepseek-harness-launcher.app
```

- 再打开应用；若仍被拦截，按住 Control 点击图标并选择「打开」。
- Gatekeeper 提示是预期行为；完成首启、Node/dsh 安装、Host 启动、托盘退出和 CLI shim 安装验证。

## Windows 测试包

- 下载 Windows x64 的 NSIS 安装包。
- SmartScreen 的未知发布者提示是预期行为；仅在受控测试机上通过「更多信息 → 仍要运行」继续。
- 完成安装、首启 Node/dsh 安装、Host 启动、托盘退出和卸载验证。

## Linux 测试包

- 下载 x64 的 `.AppImage` 或 `.deb`。
- 完成安装/启动、首启 Node/dsh 安装、Host 启动、托盘退出和卸载验证。

## 正式公开发布前

- 加入 Apple Developer Program，并为发布主体创建 Developer ID Application 证书。
- 保存 `APPLE_CERTIFICATE`、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_ID`、`APPLE_PASSWORD`、`APPLE_SIGNING_IDENTITY`、`APPLE_TEAM_ID` 和 `KEYCHAIN_PASSWORD` 为受保护的 GitHub secrets。
- 生成 Tauri updater 签名密钥；私钥仅保存为 GitHub Secret `TAURI_SIGNING_PRIVATE_KEY`，绝不提交仓库。公钥写入 updater 配置后，才能启用壳子自动更新。
- 对 macOS 包完成签名、公证和 `spctl` 验证；再准备 Windows/Linux 的签名与安装包发布。
- 确认托管 Node archive 的正式签名/分发方案；Developer ID 私钥只能留在 CI，不能放入客户端或运行时下载流程。
