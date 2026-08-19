# 发布检查清单

仓库有三条独立流水线：

- `CI`：PR 和 `main` 的测试门禁，不打包。
- `Release`：跨平台未签名测试包。
- `Docs`：把落地页发布到 GitHub Pages。

当前安装包仅供受控测试：macOS 使用 ad-hoc 签名且未公证，Windows/Linux 不签名。不要关闭系统级 Gatekeeper 或 SmartScreen。

## CI

- PR 和 `main` 上的代码变更会跑前端测试 / lint / build，以及 macOS、Windows、Linux 的 Rust clippy 与测试。
- 文档、设计稿和 workflow 文案变更不会触发 CI。

## 构建测试发布

### tag 正式草稿

1. 确认 `package.json`、`src-tauri/tauri.conf.json` 和 `src-tauri/Cargo.toml` 的版本号一致。
2. 创建并推送匹配版本的 tag：

```bash
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

3. `Release` workflow 会在测试通过后构建四平台产物，并创建 Draft + Prerelease。
4. 发布前检查 Release 说明已写明 unsigned / ad-hoc。

### 手动调试包

1. 在 Actions 手动运行 `Release`。
2. 产物只出现在该次 run 的 Artifact，保留 30 天，不会创建 GitHub Release。

## Docs / GitHub Pages

1. 仓库 Settings → Pages → Source 选择 GitHub Actions。
2. 推送 `docs/index.html`、`docs/image.png`、`docs/image-black.png` 或 `docs/launcher-mark.svg` 到 `main` 后，`Docs` workflow 会发布落地页。
3. 也可在 Actions 手动运行 `Docs`。

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
