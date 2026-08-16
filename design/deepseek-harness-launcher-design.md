# deepseek-harness-launcher 设计文档

## 1. 项目定位

**deepseek-harness-launcher** 是 [DeepSeek Harness (dsh)](https://github.com/anywhere-labs/deepseek-harness-desktop) 的轻量级桌面壳子，基于 Tauri 实现。

核心目标：

- **壳子常驻不变**：Tauri 二进制本身极少更新，体积小（~15 MB）
- **dsh 独立升级**：dsh 新版本发布后，壳子自动拉取并切换，无需重装整个应用
- **Node 运行时托管**：首次启动时自动下载 Node 到用户目录，不污染系统、不依赖用户预装
- **失败可回滚**：dsh 升级后启动失败自动回退到上个已知好版本

非目标：

- 不修改 dsh 本身的代码
- 不替代 dsh 的 Web UI，只做容器
- 不支持 dsh 之外的其他 agent harness

## 2. 与 dsh 的关系

deepseek-harness-launcher 是 dsh 的"运行环境管家"，二者通过稳定契约通信：

### 2.1 启动契约

壳子 spawn dsh CLI 子进程：

```
<node> --expose-internals <dsh-entry>/lib/bin.js web --host 127.0.0.1 --port 0
```

### 2.2 就绪协议

dsh 启动后在 stdout 输出就绪行：

```
dsh web: http://127.0.0.1:<port>/
```

壳子解析此行后拿到 origin，将 Tauri webview 导航到该 URL。约束（来自 [host-supervisor.ts](./deepseek-harness-desktop/apps/desktop/src/host-supervisor.ts#L25-L40)）：

- 协议必须为 `http:`
- hostname 必须为 `127.0.0.1` 或 `localhost`
- 必须有显式端口号（1–65535）
- pathname 必须为 `/`，无 query 和 hash

### 2.3 契约稳定性

dsh 处于开发者预览期，可能破坏性变更。壳子通过以下机制对冲：

- **版本范围锁**：默认 `~0.1.0`（只接受 patch），用户可在设置页改成 `^0.1` 或手动指定
- **启动失败回滚**：新版启动失败自动降级到 `known_good` 版本
- **engines 校验**：升级前读取 dsh 的 `package.json.engines.node`，不满足当前 Node 版本则拒绝升级

## 3. 架构

```
┌──────────────────────────────────────────────────────────┐
│                    deepseek-harness-launcher (Tauri)                       │
│                                                          │
│  ┌────────────────────┐    ┌────────────────────────┐   │
│  │   Rust 后端         │    │   Webview 前端          │   │
│  │                    │    │                        │   │
│  │  ┌──────────────┐  │    │  ┌──────────────────┐  │   │
│  │  │ Node 管理     │  │    │  │ 主界面 (dsh web) │  │   │
│  │  │ - 下载安装    │  │    │  │                  │  │   │
│  │  │ - 版本管理    │  │    │  └──────────────────┘  │   │
│  │  │ - 路径解析    │  │    │                        │   │
│  │  └──────────────┘  │    │  ┌──────────────────┐  │   │
│  │  ┌──────────────┐  │    │  │ 设置/状态页       │  │   │
│  │  │ dsh 版本管理  │  │    │  │ - dsh 版本       │  │   │
│  │  │ - 查询最新    │  │    │  │ - Node 版本      │  │   │
│  │  │ - 下载安装    │  │    │  │ - 镜像源         │  │   │
│  │  │ - 切换/回滚   │  │    │  │ - 升级策略       │  │   │
│  │  └──────────────┘  │    │  └──────────────────┘  │   │
│  │  ┌──────────────┐  │    │                        │   │
│  │  │ Host 进程监管 │  │    │  ┌──────────────────┐  │   │
│  │  │ - spawn      │  │    │  │ 首启/升级进度     │  │   │   │
│  │  │ - 就绪解析    │  │    │  │                  │  │   │
│  │  │ - 超时/崩溃   │  │    │  └──────────────────┘  │   │
│  │  └──────────────┘  │    │                        │   │
│  └────────────────────┘    └────────────────────────┘   │
└──────────────────────────────────────────────────────────┘
                         │
                         │ spawn
                         ▼
              ┌─────────────────────┐
              │  node (用户目录)     │
              │  + dsh web 子进程    │
              │  监听 127.0.0.1:port │
              └─────────────────────┘
```

## 4. 目录布局

### 4.1 安装包内容

```
deepseek-harness-launcher.app/                       (macOS)
├── Contents/
│   ├── Info.plist
│   ├── MacOS/
│   │   └── deepseek-harness-launcher            # Tauri 主二进制 (~10 MB)
│   └── Resources/
│       └── icon.icns
```

**不包含** Node、不包含 dsh。安装包只有壳子本身。

### 4.2 用户数据目录

首次运行时创建：

```
macOS:   ~/Library/Application Support/deepseek-harness-launcher/
Windows: %APPDATA%\deepseek-harness-launcher\
Linux:   ~/.local/share/deepseek-harness-launcher/

└── deepseek-harness-launcher/
    ├── state.json                  # 全局状态
    ├── node-runtime/
    │   ├── VERSION                 # "v22.19.0"
    │   ├── bin/node                # 或 node.exe
    │   ├── lib/node_modules/npm/
    │   └── ...
    ├── dsh/
    │   ├── current                 # 符号链接或 JSON 指针
    │   │   → 0.1.0-rc.6/
    │   ├── known-good              # 同上
    │   │   → 0.1.0-rc.5/
    │   ├── 0.1.0-rc.5/
    │   │   ├── package.json
    │   │   └── node_modules/@deepseek-ai/dsh/lib/bin.js
    │   └── 0.1.0-rc.6/
    │       ├── package.json
    │       └── node_modules/@deepseek-ai/dsh/lib/bin.js
    └── logs/
        └── dsh-<timestamp>.log     # dsh 子进程输出
```

### 4.3 state.json 结构

```json
{
  "schema_version": 1,
  "node": {
    "version": "v22.19.0",
    "installed_at": "2026-08-15T10:00:00Z",
    "mirror": "https://npmmirror.com/mirrors/node"
  },
  "dsh": {
    "current": "0.1.0-rc.6",
    "known_good": "0.1.0-rc.5",
    "pinned_range": "~0.1.0",
    "pending": null,
    "last_check": "2026-08-15T10:00:00Z",
    "check_interval_hours": 24,
    "registry": "https://registry.npmmirror.com",
    "installed": [
      {
        "version": "0.1.0-rc.5",
        "installed_at": "2026-08-10T10:00:00Z",
        "status": "verified"
      },
      {
        "version": "0.1.0-rc.6",
        "installed_at": "2026-08-15T10:00:00Z",
        "status": "pending"
      }
    ],
    "ignored_versions": []
  },
  "auto_upgrade": true,
  "crash_counter": 0
}
```

## 5. 核心流程

### 5.1 首次启动

```
1. 读 state.json → 不存在
2. 显示首启向导：
   "正在准备运行环境"
   ├─ 下载 Node.js v22.19.0  [██████] 25 MB
   └─ 下载 dsh 0.1.0-rc.6    [██████] 30 MB
3. Node 安装：
   - 选镜像源（默认按地区或用户选择）
   - 下载 tarball
   - 校验 SHA-256
   - 解压到 node-runtime/
   - 写 VERSION
4. dsh 安装：
   - 写 package.json: {"dependencies":{"@deepseek-ai/dsh":"0.1.0-rc.6"}}
   - spawn node npm install --prod --registry=<mirror>
   - 完整性校验 lib/bin.js 存在
   - 标记 status: "verified"
5. 标记 current = known_good = 0.1.0-rc.6
6. spawn dsh web → 拿到 URL → 开 webview
```

### 5.2 日常启动

```
1. 读 state.json
2. 检查 node-runtime/bin/node 是否存在
   - 不存在 → 进入首启修复流程
3. 检查 dsh/current/node_modules/@deepseek-ai/dsh/lib/bin.js 是否存在
   - 不存在 → 切到 known_good；known_good 也不存在 → 进入首启修复
4. 后台异步：检查 dsh 新版本（如果距 last_check > 24h）
5. spawn dsh web
6. 90 秒内解析到就绪行 → 开 webview
   90 秒超时 → 杀进程，切 known_good 重试一次
   仍失败 → 弹错误对话框，附日志路径
```

### 5.3 dsh 升级流程

```
触发：后台定时检查 / 设置页手动检查 / 启动时检查 pending

1. GET https://registry.npmjs.org/@deepseek-ai/dsh
2. 读取 dist-tags.latest
3. semver 检查：latest 是否满足 pinned_range？
   - 不满足 → 跳过（用户范围锁住了）
   - 满足 → 继续
4. latest == current？→ 跳过
5. latest 在 ignored_versions？→ 跳过
6. 读取 latest 的 package.json.engines.node
   - 当前 Node 不满足 → 提示用户升级 Node，不自动升 dsh
7. 创建 dsh/<latest>/ 目录
8. 写 package.json，spawn npm install --prod
9. 完整性校验
10. 写 state.json：pending = latest
11. 提示用户："新版本已就绪，重启后生效"
    或配置了 auto_upgrade → 立即重启
12. 下次启动时：
    - 尝试用 pending 版本启动
    - 成功 → current = pending，known_good = 旧 current，清除 pending
    - 失败 → 回滚到 known_good，记录 ignored_versions
```

### 5.4 Node 升级流程

触发：dsh 新版要求更高 Node 版本

```
1. 弹窗："dsh 0.2.0 需要 Node 24+，当前 22.19.0"
2. 用户确认 → 下载 Node 24 到 node-runtime-new/
3. 校验 SHA-256
4. 原子切换：重命名 node-runtime → node-runtime-old，node-runtime-new → node-runtime
5. 更新 state.json
6. 删除 node-runtime-old
7. 继续启动流程
```

### 5.5 崩溃恢复

```
dsh 启动后异常退出（不是启动失败，是跑了一段时间挂了）：

1. crash_counter += 1
2. 如果 crash_counter < 3 且距上次崩溃 < 5 分钟：
   - 自动重启 dsh（用 current 版本）
3. 如果 crash_counter >= 3：
   - 弹窗："当前 dsh 版本似乎不稳定"
   - 选项：[回滚到 known_good] [继续重试] [退出]
4. 用户主动重启 app → crash_counter 清零
```

## 6. 模块设计

### 6.1 Rust 后端模块

```
src-tauri/src/
├── main.rs                  # Tauri 入口
├── commands.rs              # Tauri command 暴露给前端
├── state.rs                 # AppState、state.json 读写
├── node/
│   ├── mod.rs
│   ├── download.rs          # 下载 Node tarball
│   ├── install.rs           # 解压、版本管理
│   └── version.rs           # semver 校验
├── dsh/
│   ├── mod.rs
│   ├── registry.rs          # npm registry 查询
│   ├── install.rs           # npm install 封装
│   ├── version.rs           # 版本切换、回滚
│   └── integrity.rs         # 完整性校验
├── host/
│   ├── mod.rs
│   ├── supervisor.rs        # 子进程监管（对应原 host-supervisor.ts）
│   ├── readiness.rs         # 就绪行解析
│   └── lifecycle.rs         # 启动/超时/重试
├── mirror.rs                # 镜像源管理
└── error.rs                 # 统一错误类型
```

### 6.2 前端模块

```
src/                          # React 或 Svelte
├── App.tsx
├── pages/
│   ├── Main.tsx              # 主界面：iframe 或 webview 加载 dsh web
│   ├── Settings.tsx          # 设置页
│   ├── FirstRun.tsx          # 首启向导
│   └── UpgradeDialog.tsx     # 升级提示
├── components/
│   ├── ProgressBar.tsx
│   ├── VersionBadge.tsx
│   └── MirrorSelector.tsx
└── hooks/
    ├── useDshStatus.ts
    └── useUpgrade.ts
```

## 7. 关键技术决策

### 7.1 为什么用 Tauri 而非 Electron

| | Tauri | Electron（原版 dsh-desktop） |
|---|---|---|
| 包体积 | ~15 MB | ~100 MB |
| 内存 | ~80 MB | ~200 MB |
| Node 运行时 | 不自带 | 自带（可借用作 Node） |
| 后端语言 | Rust | Node.js |
| 自动更新 | 需自己实现 | electron-updater 现成 |

选 Tauri 的代价是放弃 Electron 自带 Node，但这正是本项目的核心取舍——**用首启下载换小包体积**。

### 7.2 为什么不引导用户装系统 Node

- 普通用户不会装，劝退率高
- 系统级安装污染 PATH、可能冲突
- macOS 沙盒应用执行 `/usr/local/bin` 麻烦
- 版本管理器（nvm/fnm/asdf）路径复杂

装到用户目录是更干净的方案。

### 7.3 为什么不用 Bun/Deno 替代 Node

dsh 依赖 Node 内部模块（[vendor/loader/src/internal.ts](./deepseek-harness-desktop/vendor/loader/src/internal.ts#L103-L129)）：

```ts
require('internal/modules/esm/loader')  // 需要 --expose-internals
```

Bun/Deno 不支持此特性。详见 [vendor/hmr/src/index.ts:121](./deepseek-harness-desktop/vendor/hmr/src/index.ts#L121)。

### 7.4 为什么 dsh 安装走 npm 而非直接下 tarball

dsh 有 160 多个 workspace 依赖（见 [apps/desktop/runtime/package.json](./deepseek-harness-desktop/apps/desktop/runtime/package.json)），都是独立的 npm 包。直接下 dsh 的 tarball 装不全依赖，必须走 npm/pnpm 的依赖解析。

### 7.5 为什么用符号链接而非复制

`dsh/current` 用符号链接指向具体版本目录：

- Unix：`std::os::unix::fs::symlink`
- Windows：用 `current.json` 记录路径（符号链接需管理员权限）

切换版本 = 改符号链接指向，原子操作，不需要复制几十 MB 的 node_modules。

## 8. 安全考量

### 8.1 下载完整性

- Node tarball：校验 nodejs.org 同目录下的 `SHASUMS256.txt`
- dsh 包：用 npm registry 返回的 `dist.integrity` 字段（SHA-512）
- npm install 本身会校验每个依赖的 integrity

### 8.2 webview 安全

复用原版 [main.ts](./deepseek-harness-desktop/apps/desktop/src/main.ts#L123-L138) 的策略：

- 只允许导航到 dsh web 的 origin
- http/https 外链交给系统浏览器
- `set_permission_check_handler` 全部拒绝（摄像头、麦克风、地理位置等）
- webview 启用 `contextIsolation`、`sandbox`

### 8.3 子进程隔离

- dsh 子进程的 cwd 设为用户选择的工作目录，不是壳子目录
- 环境变量过滤：去掉壳子自己的 `RUST_*`、`TAURI_*`，只传必要的 `DSH_*`
- stdin 关闭（`stdio: ['ignore', 'pipe', 'pipe']`）

### 8.4 镜像源可信度

默认镜像源列表只放可信源：

- `https://nodejs.org/dist`
- `https://registry.npmjs.org`
- `https://npmmirror.com`（阿里）
- `https://mirrors.tuna.tsinghua.edu.cn`

允许用户自定义，但自定义源要弹窗警告。

## 9. 跨平台差异

| 平台 | Node 路径 | 符号链接 | 签名 |
|---|---|---|---|
| macOS | `node-runtime/bin/node` | `symlink(2)` | Developer ID + 公证 |
| Windows | `node-runtime\node.exe` | 用 JSON 指针 | Authenticode（待做） |
| Linux | `node-runtime/bin/node` | `symlink(2)` | 无 |

macOS 额外处理：
- 下载的 node 去除 `com.apple.quarantine` 扩展属性
- Hardened Runtime 下要给 node 二进制单独签名
- App Sandbox 要允许执行用户目录下的二进制（entitlements）

## 10. 配置项

设置页暴露：

| 配置 | 默认值 | 说明 |
|---|---|---|
| `auto_upgrade` | `true` | dsh 新版本是否自动升级 |
| `check_interval_hours` | `24` | 检查 dsh 更新间隔 |
| `pinned_range` | `~0.1.0` | dsh 版本 semver 范围 |
| `node_registry` | 按地区选 | Node 下载镜像 |
| `npm_registry` | 按地区选 | npm 安装镜像 |
| `crash_retry_limit` | `3` | 崩溃自动重试次数 |
| `keep_versions` | `2` | 保留几个 dsh 版本（current + known_good + N） |
| `working_directory` | 用户文档目录 | dsh 默认工作目录 |

## 11. 错误处理

### 11.1 用户可见错误

所有错误都给可操作的提示：

| 场景 | 提示 |
|---|---|
| 无网络 | "无法连接网络，首次启动需要下载运行环境" |
| 镜像源全失败 | "所有镜像源不可达，请检查网络或更换镜像源" |
| Node 下载损坏 | "Node 下载文件校验失败，请重试" |
| dsh 安装失败 | "dsh 安装失败（npm 错误信息），已清理" |
| dsh 启动超时 | "dsh 90 秒内未启动完成，已回滚到旧版本" |
| dsh 启动崩溃 | "dsh 启动后崩溃，已回滚。日志：<path>" |
| 磁盘空间不足 | "磁盘空间不足，需要约 200 MB" |

### 11.2 日志

- 壳子日志：`~/Library/Logs/deepseek-harness-launcher/app.log`（macOS）
- dsh 子进程日志：`<data_dir>/logs/dsh-<timestamp>.log`
- 崩溃时自动收集两份日志，提供"导出诊断信息"按钮

## 12. 发布与打包

### 12.1 CI matrix

```yaml
strategy:
  matrix:
    include:
      - os: macos-14          # arm64
        target: aarch64-apple-darwin
      - os: macos-13          # x64
        target: x86_64-apple-darwin
      - os: windows-latest
        target: x86_64-pc-windows-msvc
      - os: ubuntu-22.04
        target: x86_64-unknown-linux-gnu
```

### 12.2 产物

| 平台 | 格式 | 签名 |
|---|---|---|
| macOS | `.dmg` + `.app` | Developer ID + notarize |
| Windows | `.msi` 或 `.exe`（NSIS） | Authenticode（待做） |
| Linux | `.AppImage` + `.deb` | 无 |

### 12.3 壳子自身升级

壳子本身用 Tauri 的 `updater` 插件（`tauri-plugin-updater`）：

- 发布时同时推 `latest.json` 到 GitHub Releases
- 壳子启动时检查 `latest.json`
- 下载签名后的壳子二进制，下次启动时替换

**壳子升级和 dsh 升级独立**：壳子升级频率低（季度），dsh 升级频率高（每周或每月）。

## 13. 已知限制

1. **首次启动必须联网**——下载 Node 和 dsh。后续离线可用。
2. **dsh 的破坏性变更**——dsh 处于预览期，可能改 CLI 接口。靠版本范围锁 + 回滚兜底，但极端情况下仍需手动干预。
3. **Windows 符号链接**——普通用户权限下不能创建符号链接，用 JSON 指针替代，切换版本不是原子操作。
4. **macOS 沙盒**——如果上架 App Store，沙盒限制可能阻止执行用户目录下的 node。非 App Store 分发用 Developer ID 签名可绕过。
5. **dsh 依赖体积**——160+ 个 workspace 包，首次 `npm install` 较慢（30 秒–2 分钟），靠镜像源和进度条缓解。

## 14. 开放问题

1. 是否在安装包内预置一份初始 dsh + Node，换取离线首启能力？
   - 代价：包体积回到 ~100 MB，失去"壳子不变"优势
   - 折中：发布 "Online" 和 "Offline" 两个安装包
2. dsh 的 Web 前端（`@deepseek-ai/dsh-web-frontend`）是否随 dsh 一起升级？
   - 当前 dsh 把它列为依赖，会随 `npm install` 一起装
   - 需验证 dsh web 的前端资源路径和 dsh web 后端的兼容性
3. 是否支持多个 dsh profile？
   - 用户可能想同时跑 stable 和 canary
   - 增加复杂度，推迟到 v2

## 15. 里程碑

| 阶段 | 内容 | 产出 |
|---|---|---|
| M1: 最小可用 | Tauri 壳子 + 系统 Node + 手动装 dsh | 能跑起来 |
| M2: Node 托管 | 首启下载 Node + 版本管理 | 不依赖系统 Node |
| M3: dsh 托管 | 自动拉取 dsh + 版本切换 | 核心目标达成 |
| M4: 健壮性 | 回滚、崩溃恢复、日志、错误提示 | 可分发 |
| M5: 发布 | 签名、公证、CI、镜像源 | 对外可用 |
