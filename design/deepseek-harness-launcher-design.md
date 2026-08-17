# deepseek-harness-launcher 设计文档

## 1. 项目定位

**deepseek-harness-launcher** 是 [DeepSeek Harness (dsh)](https://github.com/anywhere-labs/deepseek-harness-desktop) 的轻量级桌面壳子，基于 Tauri 实现。

核心目标：

- **壳子常驻不变**：Tauri 二进制本身极少更新，体积小（~15 MB）
- **dsh 独立版本管理**：轻量检查 registry 的最新 dsh 版本，发现新版时提示用户，只有用户确认后才安装
- **Node 运行时托管**：首次启动时自动下载 Node 到用户目录，不污染系统、不依赖用户预装
- **失败可回滚**：dsh 切换后启动失败自动回退到上个已知好版本

非目标：

- 不修改 dsh 本身的代码
- 不替代 dsh 的 Web UI，只做容器
- 不支持 dsh 之外的其他 agent harness

产品原则：

- 主窗口只承载 dsh Web UI，不增加壳子页面和操作负担
- 新版本提示使用右侧非阻塞提示框；用户可以关闭。只有用户点击“更新”后才下载、切换并重启 dsh
- 设置页只保留版本、检查更新、更新源和诊断信息等必要操作
- 持久化只使用 `state.json` 和版本目录，不引入数据库、历史版本管理或复杂任务队列

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

壳子解析此行后拿到 origin，将 Tauri webview 导航到该 URL。约束与 dsh desktop 的 host supervisor 保持等价；上游参考实现不随本仓库分发：

- 协议必须为 `http:`
- hostname 必须为 `127.0.0.1` 或 `localhost`
- 必须有显式端口号（1–65535）
- pathname 必须为 `/`，无 query 和 hash

### 2.3 契约稳定性

dsh 处于开发者预览期，可能破坏性变更。壳子通过以下机制对冲：

- **显式更新**：首启与设置页显示当前 `latest` 的精确版本；只有用户点击安装或更新后才下载，并在安装前冻结该版本
- **启动失败回滚**：用户切换后的新版本启动失败自动降级到 `known_good` 版本
- **engines 校验**：安装或切换前读取目标 dsh 的 `package.json.engines.node`，不满足当前 Node 版本时要求用户先确认升级 Node

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
  "schema_version": 3,
  "bootstrap_plan": null,
  "node": {
    "version": "v22.19.0",
    "installed_at": "2026-08-15T10:00:00Z",
    "mirror": "https://npmmirror.com/mirrors/node"
  },
  "dsh": {
    "current": "0.1.0-rc.6",
    "known_good": "0.1.0-rc.5",
    "registry": "https://registry.npmjs.org",
    "last_notified": null,
    "installed": [
      {
        "version": "0.1.0-rc.5",
        "installed_at": "2026-08-10T10:00:00Z",
        "status": "verified"
      },
      {
        "version": "0.1.0-rc.6",
        "installed_at": "2026-08-15T10:00:00Z",
        "status": "verified"
      }
    ]
  },
  "crash_counter": 0
}
```

`bootstrap_plan` 在首启安装中保存冻结的精确 dsh 版本、Node 版本、registry、`engines.node`、解析时间与阶段；成功启动前不得用新的 `latest` 覆盖该计划。版本目录和 `state.json` 是唯一持久化来源，不使用数据库。

## 5. 核心流程

### 5.1 首次启动：原型 03 的双任务安装窗口

首启使用专用 bootstrap 界面，占据主窗口但不创建第二个 Tauri WebviewWindow。视觉和信息架构以 [原型 03](./deepseek-harness-launcher-prototype.html) 为准：标题栏、产品标识、简短说明，以及并列展示 Node 与 dsh 的两个任务卡。

首启先查询官方 npm registry 的 `dist-tags.latest`，在 dsh 任务卡上显示其当前解析值，例如 `最新版本 0.1.0-rc.6`。用户确认安装后，将该值冻结为精确版本；`latest` 仅用于查询，绝不写入安装目录或 `state.json.current`。

首启的运行时版本由用户选定 dsh 的已发布元数据决定，前端不得把 `DEFAULT_NODE_VERSION` 当成安装决策来源。`DEFAULT_NODE_VERSION` 仅是 dsh 没有声明 Node 约束时的已验证回退版本。

**运行时版本核验**：`dist-tags.latest`、`package.json.engines.node` 与 Node 发布索引都是运行时数据，不在本文档中写死版本号。若 dsh 未声明 `engines.node`，使用壳子验证过的 `DEFAULT_NODE_VERSION`，并在界面显示“dsh 未声明 Node 要求，使用已验证版本”；不得用 npm 的 `_nodeVersion` 伪称运行时兼容性要求。

```
1. 读 state.json：
   - 无 state → 显示 bootstrap 窗口，查询 registry 的 latest 并显示精确版本
   - 有未完成 bootstrap_plan → 复用该计划，不能重新读取 latest
2. 用户确认安装：
   - 将本次请求的 dist-tags.latest 解析为精确版本
   - 取得目标精确版本的 manifest，读取 engines.node 与 dist.integrity
   - 解析 Node 目标：
     - engines.node 存在且默认 Node 满足 → 选已验证的 DEFAULT_NODE_VERSION
     - engines.node 存在且默认 Node 不满足 → 从受信任 Node 发布索引选择满足范围、且当前平台有正式安装包的具体版本
     - engines.node 缺失 → 选 DEFAULT_NODE_VERSION，并标记 requirement_source = "launcher-verified-fallback"
   - 写入 state.bootstrap_plan：用户选择、精确 dsh 版本、registry、engines.node（可空）、Node 目标、解析时间与阶段
3. 渲染双任务卡：
   - Node.js v<resolved_node_version>：解析中 → 下载中 → SHA-256 校验 → 已完成
   - @deepseek-ai/dsh <resolved_dsh_version>：等待 Node → npm install → 完整性校验 → 已完成
   - 卡片显示冻结后的精确版本，不提供历史版本选择
   - 大小、百分比和剩余时间仅在数据可信时显示
4. 自动串行安装：
   - 自动选择可信 Node 镜像；仅在失败时展开“重试 / 更换镜像源”
   - 下载、校验、解压 Node，写 VERSION，将 bootstrap_plan 标记为 node_installed
   - 自动安装 bootstrap_plan.dsh_version；重试继续安装同一精确版本
5. 成功后自动启动：
   - 标记 current = known_good = <resolved_dsh_version>，清除 bootstrap_plan
   - 直接进入原型 04 的启动遮罩，spawn dsh web，收到就绪行后打开 dsh Web UI
```

镜像设置是恢复选项，不是首启门槛：默认自动选择可用可信源；解析 registry、下载 Node 或安装 dsh 失败时，在对应任务卡上提供可操作错误、重试和更换镜像源。首启不支持“使用已有系统 Node”或手动 Node 路径，保持运行时自包含、可验证和可回滚。

实现边界：Node 版本范围的求解不能依赖“提取字符串下界”的启发式逻辑。应先用 semver 校验候选版本，再由受信任的 Node 发布索引确认该版本、平台与架构的正式安装包存在。`engines.node` 缺失时不可从 npm 的 `_nodeVersion` 推导需求。

### 5.2 日常启动

```
1. 读 state.json
2. 检查 node-runtime/bin/node 是否存在
   - 不存在 → 进入首启修复流程
3. 检查 dsh/current/node_modules/@deepseek-ai/dsh/lib/bin.js 是否存在
   - 不存在 → 切到 known_good；known_good 也不存在 → 进入首启修复
4. spawn dsh web
5. 90 秒内解析到就绪行 → 开 webview
   90 秒超时 → 杀进程，切 known_good 重试一次
   仍失败 → 弹错误对话框，附日志路径
```

### 5.3 dsh 更新提示、安装与切换

应用启动后可以在后台轻量请求一次 registry 元数据，只读取 `dist-tags.latest`，不下载、不阻塞 dsh 启动。发现 `latest` 不同于当前版本时，从主窗口右侧划入非阻塞提示框。用户可以关闭提示，也可以点击按钮开始更新。设置页提供同一套“检查更新 / 更新到最新版本”入口。

```
1. 启动后轻量请求 <registry>/@deepseek-ai/dsh
   - 请求失败 → 静默忽略，不影响当前 dsh 启动
   - latest == current → 不提示
   - latest != current 且不是最近已提示的版本 → 右侧显示“发现 dsh 新版本”提示，并记录 `last_notified`
2. 用户点击“更新”：
   - 使用提示中已展示的精确版本，重新取得并校验该版本 manifest
   - 将精确版本、registry、engines.node 和 dist.integrity 冻结到本次安装请求
   - 不接受历史版本、版本范围或手动版本输入
3. 目标 == current → 显示“已是最新版本”
4. 当前 Node 不满足 engines.node → 显示简单提示，保留当前版本，不开始安装
5. 创建 dsh/<target>/，执行 npm install --prod，并校验入口与完整性
6. 安装失败 → 清理目标目录，保留 current 和 known_good；提示“更新失败，当前版本未受影响”
   - 用户可以“重试”或“更换源重试”
7. 安装成功 → 立即将 target 提升为 current，并将旧 current 记为 known_good
8. 自动重启 dsh：
   - target 启动成功 → 使用新的 current 继续当前会话
   - target 启动失败 → 回滚到 known_good，并提示“新版本无法启动，已恢复旧版本”
```

#### 可行性与边界

- **数据来源已具备**：`dsh/registry.rs` 已能读取 `dist-tags.latest` 和精确 manifest。更新提示只需要保存当前内存中的 latest，不需要历史版本列表或数据库。
- `last_notified` 只记录最近一次提示的版本，不是历史版本列表；用户关闭提示后仍可在设置页手动检查。
- **安装一致性不变**：提示只用于告知；用户点击更新后才冻结精确版本。网络重试继续使用同一目标版本，不因 registry 变化而漂移。
- **安全与回滚保持**：每个目标版本仍校验 `engines.node`、npm integrity 和入口文件；安装失败不改动当前版本。用户确认更新后，若新版本启动失败则恢复 known-good。
- **范围**：本期不提供历史版本选择、版本范围输入、后台自动下载、未经用户点击的自动重启、复杂源管理或用户可调重试参数。

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
src/
├── App.vue                    # 按 phase 切换 bootstrap、启动遮罩、dsh Web UI 和更新提示
├── components/
│   ├── MainView.vue           # dsh Web UI 容器与启动状态
│   ├── FirstRun.vue           # 原型 03 的双任务 bootstrap 界面
│   ├── Settings.vue           # 设置页
│   ├── UpdateNotice.vue       # 右侧更新提示、更新进度和重启结果
│   ├── MirrorSelector.vue     # 仅失败恢复或设置页使用
│   └── ui/                    # 自管 shadcn-vue 组件源码
├── stores/
│   └── launcher.ts            # phase、bootstrap_plan 投影、任务状态和 Host 生命周期
├── composables/
│   └── useTauriEvent.ts       # 下载、解压、dsh 安装进度事件
└── lib/
    ├── tauri.ts               # invoke 封装与 Tauri DTO
    └── format.ts
```

## 7. 关键技术决策

### 7.1 为什么用 Tauri 而非 Electron

|             | Tauri      | Electron（原版 dsh-desktop） |
| ----------- | ---------- | ---------------------------- |
| 包体积      | ~15 MB     | ~100 MB                      |
| 内存        | ~80 MB     | ~200 MB                      |
| Node 运行时 | 不自带     | 自带（可借用作 Node）        |
| 后端语言    | Rust       | Node.js                      |
| 自动更新    | 需自己实现 | electron-updater 现成        |

选 Tauri 的代价是放弃 Electron 自带 Node，但这正是本项目的核心取舍——**用首启下载换小包体积**。

### 7.2 为什么不引导用户装系统 Node

- 普通用户不会装，劝退率高
- 系统级安装污染 PATH、可能冲突
- macOS 沙盒应用执行 `/usr/local/bin` 麻烦
- 版本管理器（nvm/fnm/asdf）路径复杂

装到用户目录是更干净的方案。

### 7.3 为什么不用 Bun/Deno 替代 Node

dsh 依赖 Node 内部模块：

```ts
require("internal/modules/esm/loader"); // 需要 --expose-internals
```

Bun/Deno 不支持此特性；兼容性以 dsh 的当前发布包为准。

### 7.4 为什么 dsh 安装走 npm 而非直接下 tarball

dsh 有大量独立 npm 依赖。直接下 dsh 的 tarball 装不全依赖，必须走 npm/pnpm 的依赖解析。

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

Webview 策略与 dsh desktop 的安全边界保持等价：

- 只允许导航到 dsh web 的 origin
- http/https 外链交给系统浏览器
- `set_permission_check_handler` 全部拒绝（摄像头、麦克风、地理位置等）
- webview 启用 `contextIsolation`、`sandbox`

### 8.3 子进程隔离

- dsh 子进程的 cwd 设为其已安装版本目录；项目目录由 dsh Web UI 管理。v1 不提供 launcher 侧的工作目录配置，避免把工作区路径写入壳子状态。
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

| 平台    | Node 路径               | 符号链接     | 签名                 |
| ------- | ----------------------- | ------------ | -------------------- |
| macOS   | `node-runtime/bin/node` | `symlink(2)` | Developer ID + 公证  |
| Windows | `node-runtime\node.exe` | 用 JSON 指针 | Authenticode（待做） |
| Linux   | `node-runtime/bin/node` | `symlink(2)` | 无                   |

macOS 额外处理：

- 不主动移除下载文件的 `com.apple.quarantine` 扩展属性，保留 macOS 安全检查
- Hardened Runtime 下要给 node 二进制单独签名
- App Sandbox 要允许执行用户目录下的二进制（entitlements）

## 10. 配置项

设置页只暴露必要操作：

| 配置                | 默认值       | 说明                                          |
| ------------------- | ------------ | --------------------------------------------- |
| `node_registry`     | 自动选择     | 仅在首启失败或用户主动更换时使用              |
| `npm_registry`      | 默认 registry | 仅在 dsh 更新失败时提供更换源重试              |

`crash_retry_limit`、版本保留数量、完整性校验和回滚策略由壳子内部固定，不出现在设置页。

## 11. 错误处理

### 11.1 用户可见错误

所有错误都给可操作的提示：

| 场景          | 提示                                       |
| ------------- | ------------------------------------------ |
| 无网络        | "无法连接网络，首次启动需要下载运行环境"   |
| 镜像源全失败  | "所有镜像源不可达，请检查网络或更换镜像源" |
| Node 下载损坏 | "Node 下载文件校验失败，请重试"            |
| dsh 更新失败  | "更新失败，当前版本未受影响。你可以重试或更换源" |
| dsh 启动超时  | "dsh 90 秒内未启动完成，已回滚到旧版本"    |
| dsh 启动崩溃  | "dsh 启动后崩溃，已回滚。日志：<path>"     |
| 磁盘空间不足  | "磁盘空间不足，需要约 200 MB"              |

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
      - os: macos-14 # arm64
        target: aarch64-apple-darwin
      - os: macos-13 # x64
        target: x86_64-apple-darwin
      - os: windows-latest
        target: x86_64-pc-windows-msvc
      - os: ubuntu-22.04
        target: x86_64-unknown-linux-gnu
```

### 12.2 产物

| 平台    | 格式                     | 签名                    |
| ------- | ------------------------ | ----------------------- |
| macOS   | `.dmg` + `.app`          | Developer ID + notarize |
| Windows | `.msi` 或 `.exe`（NSIS） | Authenticode（待做）    |
| Linux   | `.AppImage` + `.deb`     | 无                      |

### 12.3 壳子自身升级

壳子本身用 Tauri 的 `updater` 插件（`tauri-plugin-updater`）：

- 发布时同时推 `latest.json` 到 GitHub Releases
- 壳子启动时检查 `latest.json`
- 下载签名后的壳子二进制，下次启动时替换

**壳子升级和 dsh 升级独立**：壳子升级频率低（季度），dsh 升级频率高（每周或每月）。

## 13. 已知限制

1. **首次启动必须联网**——下载 Node 和 dsh。后续离线可用。
2. **dsh 的破坏性变更**——dsh 处于预览期，可能改 CLI 接口。显式安装最新版本、`engines.node` 校验与回滚能降低风险，但极端情况下仍需手动干预。
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

| 阶段          | 内容                                | 产出            |
| ------------- | ----------------------------------- | --------------- |
| M1: 最小可用  | Tauri 壳子 + 系统 Node + 手动装 dsh | 能跑起来        |
| M2: Node 托管 | 首启下载 Node + 版本管理            | 不依赖系统 Node |
| M3: dsh 托管  | 显式选择、安装 dsh + 版本切换       | 核心目标达成    |
| M4: 健壮性    | 回滚、崩溃恢复、日志、错误提示      | 可分发          |
| M5: 发布      | 签名、公证、CI、镜像源              | 对外可用        |
