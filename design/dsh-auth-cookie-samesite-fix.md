# dsh Web 认证 Cookie 跨站修复记录

> 状态：已修复。本文记录 2026-09-05 排查并修复「WebView 内嵌 dsh 显示 authentication required」问题的完整过程：现象、证据、根因、修复方案与验证方式。相关前置修复（就绪行解析允许 `?token=`）见提交 `0481ce5`。

## 1. 问题现象

dsh `0.1.2-rc.1` 的就绪行带上了启动令牌：

```text
dsh web: http://127.0.0.1:<port>/?token=<launch-token>
```

壳子解析该 URL 后将其作为 iframe `src` 打开，WebView 内显示：

```text
dsh web authentication required; reopen the URL printed by dsh web.
```

直接在终端 `curl` 同一 URL 返回 `303 See Other` + `Set-Cookie`（`HttpOnly; SameSite=Strict`），证明令牌有效、服务端认证流程本身没问题，失败发生在 WebView 内部。

## 2. dsh 的认证协议

源码位置：安装目录下 `@deepseek-ai/dsh-client-connection/lib/index.js`（`BrowserAuth`）。

1. 每个进程启动时生成随机 `launchToken`，打印带 `?token=` 的根 URL。
2. `GET /?token=...` 且令牌匹配 → 返回 `303 Location: /`，同时 `Set-Cookie`：绑定 authority（host:port）的签名 cookie，`Path=/; HttpOnly; SameSite=Strict`，有效期默认 30 天。
3. 重定向后的 `GET /` 及所有后续 RPC 请求靠该 cookie 认证；缺失即 `401`。

关键约束：

- cookie 是 `SameSite=Strict` 的 host-only cookie；
- dsh 侧没有免认证或关闭认证的选项（README 明确「HTTP 载体不在根路径交换之外接受 query token，也不接受 Authorization header token」）。

## 3. 根因

通过临时埋点（iframe 事件监听 + 注入脚本 postMessage 上报）拿到运行时证据：

```text
iframe src 赋值:  url=http://127.0.0.1:53053/?token=...  hasToken=true
iframe load:      url=http://127.0.0.1:53053/             hasToken=false
dsh document:     url=http://127.0.0.1:53053/  authenticationRequired=true
```

完整链路：

1. 壳前端把带 token 的完整 URL 交给 iframe（token 没有丢失）；
2. WebView 请求 `/?token=...`，dsh 校验通过并返回 `303` + `Set-Cookie`；
3. WebView 跟随重定向请求 `/`；
4. 该请求**没有携带刚下发的 cookie** → dsh 返回 `401`。

原因在于 SameSite 的站点计算：

- 壳的生产页面运行在 `tauri.localhost`（Tauri 内置协议的 origin）；
- dsh iframe 运行在 `127.0.0.1:<port>`；
- `tauri.localhost` 与 `127.0.0.1` 是两个不同的 site，iframe 是跨站（cross-site）上下文；
- WebKit（macOS WKWebView）在跨站 iframe 场景下按 ITP 规则拒绝存储/回发 `SameSite=Strict` cookie，于是重定向后的请求永远缺 cookie。

即：**壳页与 dsh 不在同一站点，导致 dsh 强制的浏览器会话 cookie 在 iframe 中失效**。

## 4. 修复方案

让壳前端与 dsh 处于同一 site（`127.0.0.1`），cookie 退化为第一方 cookie，WKWebView 正常存储回发。dsh 固定以 `--host 127.0.0.1` 启动，因此把前端也搬到 `127.0.0.1` 的随机端口上。

### 4.1 生产模式：tauri-plugin-localhost 提供前端回环服务

- 新增依赖 `tauri-plugin-localhost` 与 `portpicker`；
- 启动时 `portpicker::pick_unused_port()` 选随机空闲端口，插件以 `.host("127.0.0.1")` 把 `frontendDist` 资产 serve 在 `http://127.0.0.1:<port>`；
- 主窗口从 `tauri.conf.json` 静态配置改为 `app.rs` 内 `WebviewWindowBuilder` 动态创建，加载 `WebviewUrl::External(前端回环 URL)`（尺寸/标题/主题等配置从 JSON 平移到代码）；
- 由于前端 origin 是运行时随机端口，静态 capability 文件无法预写：开启 tauri `dynamic-acl` feature，在 setup 中用 `CapabilityBuilder::new("loopback-frontend").remote(url).window("main")` 动态授予 IPC 权限。该 capability 默认 `local=true`，同时覆盖 dev 与生产上下文，因此原静态 `capabilities/default.json` 已删除，动态 capability 是唯一授权来源；
- `tauri.conf.json` 中 `app.windows` 置空，`devUrl` 改为 `http://127.0.0.1:1420`；dev 模式的前端 origin 直接读取该 `devUrl`，不重复定义。

插件在独立线程起服务器，`setup` 中以 `wait_for_loopback_server`（最长 3 秒的 TCP 探活循环）确认端口就绪后再创建窗口，避免首帧白屏；探活超时改为弹出可操作的错误对话框并退出，而不是加载死 URL 留下白屏。

### 4.2 开发模式：devUrl 同样指向 127.0.0.1

- `vite.config.ts` 的 `server.host` 默认值从 `false` 改为 `"127.0.0.1"`（`TAURI_DEV_HOST` 覆盖逻辑保留）；
- `tauri.conf.json` 的 `devUrl` 从 `http://localhost:1420` 改为 `http://127.0.0.1:1420`——注意 `localhost` 与 `127.0.0.1` 在 SameSite 语义下是不同 site，必须用 IP；
- dev 模式不启用 localhost 插件，窗口仍走 `WebviewUrl::App("/")` 由 devUrl 承载。

### 4.3 导航策略适配

`navigation.rs`：

- `NavigationPolicy` 新增 `launcher_origin` 槽位，`activate_launcher_origin` 在窗口创建前注册当前前端 origin（dev 的 `127.0.0.1:1420` 或生产的随机端口），`allows()` 对其放行；
- 原 `is_launcher_url` 中 `cfg!(debug_assertions)` 放行 `localhost:1420` 的特判删除（dev origin 现在经 `launcher_origin` 注册），仅保留 `tauri://localhost` 与 `tauri.localhost` 两个内置协议 origin；
- dsh origin 的校验逻辑不变。

## 5. 涉及文件

| 文件 | 改动 |
| --- | --- |
| `src-tauri/Cargo.toml` | 新增 `tauri-plugin-localhost`、`portpicker`；tauri 加 `dynamic-acl` feature |
| `src-tauri/tauri.conf.json` | `windows: []`；`devUrl` 改 `http://127.0.0.1:1420` |
| `src-tauri/src/ipc_commands.rs` | 新增：IPC 命令清单单一事实源，配套测试防止与 `generate_handler!` 漂移 |
| `src-tauri/build.rs` | 由清单生成 `allow-<command>` ACL 权限 |
| `src-tauri/src/app.rs` | 动态创建主窗口；生产模式接 localhost 插件 + 动态 capability + 端口探活；dev/prod 均注册 launcher origin |
| `src-tauri/src/navigation.rs` | `launcher_origin` 放行逻辑与配套测试 |
| `vite.config.ts` | dev server 默认绑定 `127.0.0.1` |

## 6. 验证

- Rust 单测 209 通过（含新增 `allows_only_the_registered_frontend_origin` 与 IPC 命令清单防漂移测试）、集成测试 4 通过；
- 本地安装新 `.app` 后：iframe 加载 `/?token=...` → `303` → `/`，dsh 界面正常渲染，无 `401`；
- 回归点：托盘「在浏览器中打开」仍用外部默认浏览器（浏览器顶层访问 `127.0.0.1` 时 cookie 为第一方，不受影响）。

## 7. 遗留说明

- 生产模式前端资产由本机回环 HTTP 明文提供，仅监听 `127.0.0.1`，与 dsh 自身的安全模型一致；
- 前端端口每次启动随机，静态 capability 无法覆盖，故依赖 `dynamic-acl` 运行时授权——若后续 Tauri 收紧该能力，需评估固定端口 + 静态 capability 的替代方案；
- 设计文档 §2.2 的就绪行契约已由 dsh `0.1.2-rc.1` 单方面扩展（带 `?token=`），壳侧解析已适配，设计文档 §2.2 已同步该契约。
