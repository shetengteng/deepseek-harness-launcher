# 黑白主题实施方案

## 目标与边界

为启动器增加可持久化的黑白主题切换：

- 浅色：白色背景、黑色文字；
- 深色：黑色背景、白色文字；
- 用户在“设置”中切换后立即生效，重启应用后保持选择；
- 在首次安装运行环境的页面也可随时切换；
- 本期不增加“跟随系统”、自定义色彩或影响内嵌 dsh 网页的主题。内嵌网页仍由 dsh 自己控制。

这里的“黑白主题”指启动器本身的浅色/深色两套单色界面，而不是将图标、图片或 dsh 内容强制转换为灰度。

### dsh 隔离调研结论

本功能**不会穿透或修改 dsh 的主题和设置**，并且应保持这一边界：

- launcher 将 dsh 加载为 `http://127.0.0.1:<随机端口>/` 的 iframe；其 origin 与 Tauri 壳页不同，CSS class 与 CSS 变量不会跨 iframe 文档继承。
- iframe 使用 `sandbox`，现有跨帧逻辑仅允许 dsh 向壳页发送经过来源校验的 `dsh:open-external` 外链消息。代码中不存在父页向 dsh 发送主题消息、读取 iframe DOM 或注入主题样式的路径。
- launcher 的 `state.json` 只保存壳子运行时和设置；dsh 项目目录及其 UI 设置不由 launcher 管理。
- 实现时只切换 launcher 根元素的 `.dark` class，不修改 `src-tauri/tauri.conf.json` 的原生窗口主题，也不新增 `postMessage` 或注入脚本来传递主题。

若将来产品希望同步 dsh 主题，必须另行设计由 dsh 明确支持、带版本与来源校验的消息协议；不能通过 CSS 穿透或未经协商的脚本注入实现。

## 现状

- `src/styles.css` 已有默认色板和 `.dark` 色板，Tailwind 也启用了 `darkMode: ["class"]`。
- `index.html` 现在会在挂载前读取主题缓存并设置 `.dark`，应用运行时再由主题 store 与 Rust 状态同步。
- 设置页由 `src/components/Settings.vue` 组合多个卡片；仓库已有可复用的 `Switch` 组件。
- 需要跨重启保留的设置当前保存在 Rust 端的 `state.json`，其读取和原子写入由 `src-tauri/src/state/` 负责。
- 首启状态即使已存在不完整的 `state.json`，仍会由缺少 Node/dsh 的状态判定为 `first_run`，因此可以安全持久化用户在安装页选择的主题。

## 交互与状态契约

在设置页顶部增加“外观”卡片：

| 项目 | 约定 |
| --- | --- |
| 标题 | `外观` |
| 控件 | 一个带可访问名称的 `Switch` |
| 关闭状态 | `浅色主题`，说明为“白色背景，黑色文字” |
| 打开状态 | `黑白主题`，说明为“黑色背景，白色文字” |
| 生效时机 | 用户点击后立即更新根元素样式；保存失败时恢复原主题并显示可操作错误 |
| 默认值 | `light`（浅色，白色背景） |

持久化字段采用封闭枚举而不是布尔值，便于未来在不破坏数据的情况下加入 `system`：

```json
{
  "theme": "light"
}
```

本期允许值仅为 `light` 和 `dark`。缺少字段的旧 `state.json` 通过 Serde 默认值视为 `light`，以使用白色主题作为默认界面；不需要提升 `schema_version`，也不应影响现有运行时、镜像源和 dsh 状态。

首次安装页采用独立但等价的主题切换按键：置于安装卡片右上角，使用可见文案和太阳/月亮图标表示“切换为浅色”或“切换为黑白主题”。它不依赖设置弹窗，下载、解压和重试期间始终可用，且具有明确的 `aria-label`。

## 实施工作

1. 扩展持久化设置。
   - 在 `src-tauri/src/state/types.rs` 定义 `ThemeMode`（`light` / `dark`）并为其实现默认值 `light`。
   - 在 `AppState` 增加带 `#[serde(default)]` 的 `theme` 字段；从 `src-tauri/src/state.rs` 重新导出该类型。
   - 保留现有原子写入策略。旧状态文件读入后，只有用户切换主题或其他设置被保存时才会写出新增字段。

2. 提供窄的 Tauri 设置接口。
   - 在 `src-tauri/src/commands/settings.rs` 添加 `get_theme_command` 和 `set_theme_command`；后者只接受枚举定义的两个值并保存状态。
   - 在 `src-tauri/src/commands.rs` 重导出命令，并在 `src-tauri/src/app.rs` 的 `generate_handler!` 注册。
   - 不复用 `get_dsh_state` 作为全局主题初始化接口，避免让启动时的外观依赖设置弹窗的数据加载。

3. 建立前端主题状态与根元素同步。
   - 在 `src/lib/tauri.ts` 声明 `ThemeMode` 类型以及 `getTheme`、`setTheme` 封装。
   - 新建 `src/composables/useTheme.ts`（或等价的专用 Pinia store），集中负责读取偏好、切换 `document.documentElement` 的 `.dark` 类、保存选择，以及保存失败后的回滚。
   - 在应用挂载时初始化该状态，设置弹窗关闭时不应重置主题。
   - 将 `index.html` 中写死的 `class="dark"` 改为在 `<head>` 中读取同名的只读前端缓存；缓存缺失时默认 `light`。Tauri 返回的 `state.json` 值是最终权威值，初始化后必须纠正缓存和 DOM，避免主题闪烁。

4. 增加设置页控件。
   - 新建 `src/components/settings/SettingsAppearanceCard.vue`，复用 `Card`、`Label` 和 `Switch`，仅接收当前值、保存中状态和错误信息，并通过事件请求切换。
   - 在 `src/components/Settings.vue` 的首个卡片之前插入外观卡片，接入全局主题状态并处理乐观更新、失败回滚与错误展示。
   - 使用现有语义色 token（`background`、`foreground`、`card`、`muted`、`border` 等），不写死黑白色值；必要时只补齐现有 token 未覆盖的组件状态。

5. 增加首次安装页的主题按键。
   - 在 `src/components/FirstRun.vue` 使用同一个 `useTheme` 状态，不复制保存、回滚或 DOM class 的逻辑。
   - 将按键放在安装卡片右上角，不遮挡产品图标、进度、下载源切换和重试操作；主题保存中禁用重复点击。
   - 首启时 `set_theme_command` 可以创建只含默认运行时字段和主题字段的 `AppState`。必须确认 `launcher_status`、`resolve_bootstrap_plan_command` 与后续安装都继续识别为 `first_run` 并保留该主题。

6. 检查宿主界面覆盖范围。
   - 逐页验证首启向导、启动/空闲页、设置与其确认弹窗、关于页、错误/崩溃/更新弹窗、Toast 和 Host 启动遮罩。
   - 确认 iframe 外围容器跟随主题，而 iframe 内的 dsh 页面不被 CSS 穿透或强制改色。
   - 不修改 dsh iframe 的 `src`、sandbox/权限配置或现有 `dsh:open-external` 单向桥接；主题切换不发送跨帧消息。

## 测试与验收

### Rust 单元测试

- `ThemeMode` 的默认值为 `light`，`light` 与 `dark` 都能反序列化。
- 缺少 `theme` 字段的当前 `state.json` 可以读取，并得到 `light`。
- `set_theme_command` 写入后，重新读取得到相同值；非法值返回结构化错误且不改写状态。
- 无状态文件的首次安装页切换主题后，`launcher_status` 仍返回 `first_run`，且 `resolve_bootstrap_plan_command` 保留已选主题。

### 前端单元测试

- `useTheme` 初始化后，`light` 不含 `.dark`、`dark` 含 `.dark`。
- 切换时先更新页面，再调用 `setTheme`；调用失败会恢复原 class 和可见值并显示错误。
- 外观卡的开关具有正确标签和当前主题文案。
- 首次安装页始终显示主题按键；按键调用共享主题状态，下载中切换不打断进度、保存失败能回滚。
- 主题切换测试确认只改 launcher 的根元素 class：不访问 `iframe.contentDocument`，不改变 iframe `src`，也不向 dsh 发送 `postMessage`。
- `Settings` 加载、错误与交互测试保留现有下载源、Node 更新、dsh 更新和卸载行为。

### 手工验收

1. 首次启动默认显示白色主题，安装卡片右上角的主题按键状态正确。
2. 在 Node/dsh 下载、解压或重试期间切换主题，进度与安装任务不中断；关闭并重开应用后，仍使用最后一次选择。
3. 开启“黑白主题”后，设置弹窗、后台页面和后续弹窗均为黑底白字；关闭后恢复白底黑字。
4. 切换主题不重启 dsh、不改变当前 iframe 地址，也不影响下载源和运行时设置。
5. 切换 launcher 主题前后，dsh iframe 内的配色、路由和已有用户设置均不变。
6. 在浅色和深色下键盘焦点、禁用状态、错误文字和对话框边界均清晰可辨。

## 交付物与执行顺序

建议按“状态与命令 → 前端主题状态 → 设置卡片 → 测试与全页验收”的顺序实现。交付包含实现代码、上述 Rust/Vitest 测试，以及本文件的完成状态更新；不修改 `design/` 下的既有只读设计文档。
