# 发布检查清单

此清单适用于 GitHub Actions 的 `Draft macOS test release` 工作流。它只构建 arm64 与 x64 的 macOS 测试包，使用 ad-hoc 签名，不需要 Apple Developer 证书，也不会公证。

测试包仅供受控测试，必须保持 GitHub 草稿 Release 和 prerelease 状态，不能作为公开分发版本。

## macOS 测试包

- 在 Actions 手动运行 `Draft macOS test release`，并在确认输入框填写 `macos-test`。
- 下载草稿中的 `.dmg` 后，分别在 Apple Silicon 与 Intel Mac 上完成首启、Node/dsh 安装、Host 启动、托盘退出和 CLI shim 安装验证。
- Gatekeeper 提示是预期行为；仅从本项目的 GitHub 草稿下载测试包，不要将其转发给外部用户。

## 正式公开发布前

- 加入 Apple Developer Program，并为发布主体创建 Developer ID Application 证书。
- 保存 `APPLE_CERTIFICATE`、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_ID`、`APPLE_PASSWORD`、`APPLE_SIGNING_IDENTITY`、`APPLE_TEAM_ID` 和 `KEYCHAIN_PASSWORD` 为受保护的 GitHub secrets。
- 生成 Tauri updater 签名密钥；私钥仅保存为 GitHub Secret `TAURI_SIGNING_PRIVATE_KEY`，绝不提交仓库。公钥写入 updater 配置后，才能启用壳子自动更新。
- 对 macOS 包完成签名、公证和 `spctl` 验证；再准备 Windows/Linux 的签名与安装包发布。
