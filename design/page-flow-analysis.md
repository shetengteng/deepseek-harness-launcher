# 页面流转分析与修复方案（历史记录）

> 状态：已归档。本文记录 2026-08-16 冒烟测试中暴露的“错误关闭后陷入循环”及当时的修复提案；当前状态机、测试门禁和待办以 [实施计划](./deepseek-harness-launcher-implementation-plan.md) 与 [测试计划](./deepseek-harness-launcher-test-plan.md) 为准。文中的未勾选项不再表示当前 TODO。

## 1. 现状：当前状态机

### 1.1 顶层 phase（`MainView.vue` 渲染分支）

```
booting ──refreshStatus──> first_run | idle | ready
                                │       │
                                └──error──┘
```

| phase      | 视图             | 触发条件                                              |
| ---------- | ---------------- | ----------------------------------------------------- |
| `booting`  | Loading spinner  | App 启动初始值                                        |
| `first_run`| FirstRun 向导    | `launcher_status` 返回 `phase=first_run`              |
| `idle`     | 卡片 + 按钮       | `launcher_status` 返回 `phase=idle`                   |
| `ready`    | iframe(dsh web)  | `startHost` 成功                                      |
| `error`    | ErrorDialog 覆盖 | 任何 action 抛错                                       |

### 1.2 FirstRun 子状态 `wizardStep`

```
mirror_select → probing → downloading → extracting → done
     │              │           │             │          │
     └──────────────┴───────────┴─────────────┴── failed
```

### 1.3 后端 `build_status_snapshot` 派生 phase 的规则（修复前）

```rust
StateStatus::FirstRun           → phase = "first_run"   // state.json 不存在
StateStatus::Loaded(state)      → phase = "idle"        // state.json 存在
```

**问题**：只看 state.json 是否存在，不看 Node/dsh 是否装完。

## 2. 测试中暴露的循环问题

### 2.1 场景 A：Node 已装、dsh 未装

**复现步骤**：

1. 首启 → FirstRun 向导 → 装 Node 完成 → `wizardStep=done`，显示"安装 dsh"按钮
2. 用户点"安装 dsh" → `install_dsh_command` 失败（网络/io/npm 错误）
3. `fail(e)` → `phase=error`
4. ErrorDialog 关闭 → `resetError` 看 `kind` 不是 `node_not_installed`/`dsh_not_installed`
5. 切回 `phase=idle`
6. idle 视图显示"安装 dsh"按钮（因为 `dshVersion=null`）
7. 用户再点 → 再失败 → 回到步骤 3 → **死循环**

### 2.2 场景 B：dsh 未装但用户点"启动 Host"

1. idle 视图：`dshVersion=null`，按钮逻辑只会显示"安装 dsh"，不会显示"启动 Host"
2. 但若 `resetError` 错误地切到 idle，用户进入 idle 后看到"安装 dsh"，又走场景 A

### 2.3 场景 C：state.json 残留（Node 字段为空）

1. 上次测试中断，state.json 已写入但 `node=None`、`dsh.current=None`
2. `build_status_snapshot` 返回 `phase=idle`
3. idle 视图显示"dsh 版本：未安装 / Node 版本：未托管 / 安装 dsh"按钮
4. 用户点"安装 dsh" → `install_dsh_command` 检查 `state.node` 为 None，返回 `NodeNotInstalled`
5. ErrorDialog 关闭 → `resetError` 切回 `first_run`（kind=node_not_installed）
6. FirstRun 向导重新进入，但 `wizardStep` 仍是初始 `mirror_select`，用户以为要重装 Node

### 2.4 根因总结

| 根因 | 描述 |
| --- | --- |
| R1 | `build_status_snapshot` 用 state.json 存在性决定 phase，不区分 Node/dsh 装没装 |
| R2 | `resetError` 仅看 `error.kind` 决定切到 first_run/idle，丢失了"错误发生在哪个操作"的上下文 |
| R3 | `install_dsh` 失败后没有明确的恢复路径，用户只能"关闭后回到 idle 再点一次" |
| R4 | ErrorDialog 的"重试"按钮硬编码调 `startHost`，与 `installDsh` 失败场景不匹配 |
| R5 | FirstRun 子状态 `wizardStep` 与顶层 `phase` 耦合不清晰，错误后切回 first_run 时子状态可能错位 |

## 3. 目标设计

### 3.1 状态机原则

**原则 1：错误后停留在错误发生的视图，由用户主动选择下一步。**

- 不自动切 phase
- ErrorDialog 提供"重试（同操作）"和"返回上一步"两个明确选项

**原则 2：phase 派生应反映真实可用性，而非 state.json 存在性。**

- Node 未装 → `first_run`（无论 state.json 是否存在）
- Node 已装、dsh 未装 → `first_run` + `wizardStep=done`（停在"安装 dsh"按钮）
- Node 已装、dsh 已装 → `idle`（可启动 Host）

**原则 3：FirstRun 子状态由 store 显式管理，不依赖 phase 派生。**

- 错误恢复后进入 first_run 时，根据 `nodeVersion`/`dshVersion` 决定 `wizardStep`：
  - 都未装 → `mirror_select`
  - Node 已装、dsh 未装 → `done`（显示"安装 dsh"按钮）

### 3.2 目标流转图

```
                            ┌─────────────────────────────┐
                            │      App 启动 (booting)      │
                            └──────────┬──────────────────┘
                                       │ refreshStatus
                                       ▼
                       ┌───────────────────────────────┐
                       │ build_status_snapshot 派生    │
                       │  • node=None         → first_run (wizardStep=mirror_select)
                       │  • node ok, dsh=None → first_run (wizardStep=done)
                       │  • node ok, dsh ok    → idle
                       └──────────┬────────────────────┘
                                  │
            ┌─────────────────────┼─────────────────────┐
            ▼                     ▼                     ▼
     ┌────────────┐        ┌────────────┐         ┌──────────┐
     │ mirror_    │        │   done     │         │   idle   │
     │ select     │        │ (Node ok,  │         │ (dsh ok) │
     │            │        │  dsh=None) │         │          │
     └─────┬──────┘        └─────┬──────┘         └────┬─────┘
           │ installNode         │ installDsh           │ startHost
           ▼                     ▼                       ▼
     ┌───────────┐         ┌───────────┐          ┌───────────┐
     │downloading│         │installing │          │ starting  │
     └─────┬─────┘         │    dsh    │          └─────┬─────┘
           │               └─────┬─────┘                │
           ▼                     │                      ▼
     ┌───────────┐               │ refreshStatus   ┌─────────┐
     │extracting │               │ (dsh.current    │  ready  │
     └─────┬─────┘               │  写入)          │ (iframe)│
           │                     ▼                 └─────────┘
           ▼               ┌───────────┐
     ┌───────────┐         │   idle    │
     │   done    │◀────────│ (dsh ok)  │
     └───────────┘         └───────────┘
```

### 3.3 错误恢复矩阵

| 错误发生位置 | `error.kind` 例 | ErrorDialog 按钮 | 关闭后 phase |
| --- | --- | --- | --- |
| FirstRun 装 Node | `node_download_failed` / `io` | 重试(installNode) / 返回(mirror_select) | `first_run` + `wizardStep=failed` |
| FirstRun 装 dsh | `dsh_install_failed` / `io` / `network` | 重试(installDsh) / 关闭 | `first_run` + `wizardStep=done` |
| idle 启动 Host | `node_not_installed` / `dsh_not_installed` | 去安装 | `first_run` + 对应 wizardStep |
| idle 启动 Host | `host_spawn_failed` / `io` / `timeout` | 重试(startHost) / 关闭 | `idle` |
| ready 运行中崩溃 | `host_unexpected_exit` | 重启(startHost) / 关闭 | `idle` |

### 3.4 错误上下文携带

store 新增字段记录"上一次失败的操作"：

```ts
type LastAction = "installNode" | "installDsh" | "startHost" | null
const lastFailedAction = ref<LastAction>(null)

function fail(e, action: LastAction) {
  error.value = normalizeError(e)
  lastFailedAction.value = action
  // phase 切到 error，但保留 preErrorPhase 用于"关闭"恢复
  preErrorPhase.value = phase.value
  preErrorWizardStep.value = wizardStep.value
  phase.value = "error"
}
```

ErrorDialog 根据 `lastFailedAction` 决定"重试"按钮调哪个 action：

```vue
<Button @click="onRetry">
  重试 {{ lastFailedAction === 'installDsh' ? '安装 dsh' : '启动 Host' }}
</Button>
```

### 3.5 `resetError` 新逻辑

```ts
function resetError(): void {
  if (phase.value !== "error") return
  const kind = error.value?.kind
  error.value = null

  // 1. Node/dsh 未装：强制 first_run，根据 nodeVersion 选 wizardStep
  if (kind === "node_not_installed" || kind === "dsh_not_installed") {
    phase.value = "first_run"
    wizardStep.value = nodeVersion.value ? "done" : "mirror_select"
    return
  }

  // 2. 其他错误：恢复到错误前的 phase/wizardStep
  phase.value = preErrorPhase.value
  wizardStep.value = preErrorWizardStep.value
  preErrorPhase.value = null
  preErrorWizardStep.value = null
}
```

### 3.6 后端 `build_status_snapshot` 新规则

```rust
StateStatus::FirstRun => phase = "first_run"
StateStatus::Loaded(state) => match (state.node.as_ref(), state.dsh.current.as_deref()) {
    (None, _) => phase = "first_run",   // Node 没装
    (Some(_), None) => phase = "first_run",  // dsh 没装
    (Some(_), Some(_)) => phase = "idle",
}
```

前端 `applySnapshot` 收到 `phase=first_run` 时，根据 `nodeVersion`/`dshVersion` 设 `wizardStep`：

```ts
if (snap.phase === "first_run") {
  phase.value = "first_run"
  if (snap.node_version && snap.dsh_version) {
    // 极端情况：后端认为是 first_run 但 dsh 已装，切到 idle
    phase.value = "idle"
  } else if (snap.node_version) {
    wizardStep.value = "done"  // Node 已装，停在"安装 dsh"
  } else {
    wizardStep.value = "mirror_select"  // 从头开始
  }
  return
}
```

## 4. 实施清单

### 4.1 后端

- [x] `error.rs`：现有 `NodeDownload(String)` / `DshInstall(String)` 已能区分安装失败（kind=`node_download` / `dsh_install`），`start_host` 失败走 `Host(String)` / `NodeNotInstalled` / `DshNotInstalled`。前端用 store 自己记录的 `lastFailedAction` 区分操作来源，**无需新增错误变体**。
- [ ] `commands.rs::build_status_snapshot`：Node 或 dsh 未装均返回 `first_run`

### 4.2 前端 store

- [ ] 新增 `lastFailedAction`、`preErrorPhase`、`preErrorWizardStep` 字段
- [ ] `fail(e, action)` 签名改造，记录上下文
- [ ] `resetError` 改为按 §3.5 逻辑
- [ ] `applySnapshot` 按 §3.6 设 `wizardStep`
- [ ] `installNode` / `installDsh` / `startHost` 调用 `fail(e, ...)` 时传入对应 action

### 4.3 前端组件

- [ ] `ErrorDialog.vue`："重试"按钮根据 `lastFailedAction` 调对应 action；非 `startHost` 错误时"重试"文案改为对应操作名
- [ ] `MainView.vue`：移除硬编码 `store.startHost()` 的 retry，改为调 `store.retryLastAction()`
- [ ] `FirstRun.vue`：`wizardStep=failed` 时根据 `lastFailedAction` 显示不同重试按钮

### 4.4 验证

- [ ] 场景 A：装 dsh 失败后，关闭错误对话框应停留在 FirstRun 的 done 步骤，可重新点"安装 dsh"
- [ ] 场景 B：启动 Host 失败（io error），关闭后应停留在 idle，可重新点"启动 Host"
- [ ] 场景 C：state.json 残留 Node=None，启动应进 FirstRun 的 mirror_select
- [ ] 场景 D：state.json 残留 Node=ok、dsh=None，启动应进 FirstRun 的 done 步骤

## 5. 风险与边界

- **风险 1**：`preErrorPhase` 在嵌套错误时可能丢失。当前设计不支持错误嵌套，每次 `fail` 覆盖上一次上下文。
- **风险 2**：用户在 FirstRun 装 Node 中途关闭应用，下次启动 `wizardStep` 应该重置为 `mirror_select`，不应误以为已装。后端 `build_status_snapshot` 已能识别 Node=None 场景。
- **边界**：本方案不引入"撤销安装"功能，已装的 Node/dsh 版本只能通过后续 PR 提供的"清理"命令移除。
