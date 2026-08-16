// Pinia launcher store。对应设计 §M1.5 + §M2.5（PR-011 首启向导）。
// 状态机：`booting → first_run | idle | ready | error`，actions 调 `lib/tauri.ts`。
// 首启向导子状态机：`wizard_step = mirror_select | downloading | extracting | done | failed`。

import { defineStore } from "pinia";
import { computed, ref, shallowRef } from "vue";

import {
  fetchStatus,
  installDsh,
  installNode,
  listMirrors,
  probeMirrors,
  restartHost,
  rollbackDsh,
  startHost,
  shutdownHost,
  validateCustomMirror,
  type CrashLimitPayload,
  type HostRestartedPayload,
  type LauncherErrorPayload,
  type MirrorInfo,
  type ProgressEvent,
  type StatusSnapshot,
} from "@/lib/tauri";

/** 前端状态机的 phase。对应 `MainView.vue` 的视图分支。 */
export type LauncherPhase = "booting" | "first_run" | "idle" | "ready" | "error";

/** 首启向导步骤。顺序流转：`mirror_select → downloading → extracting → done`，任意步骤可 `failed`。 */
export type WizardStep =
  | "mirror_select"
  | "probing"
  | "downloading"
  | "extracting"
  | "done"
  | "failed";

/** 默认 Node 版本（设计 §M2.4）。与 Rust `DEFAULT_NODE_VERSION` 对齐。 */
export const DEFAULT_NODE_VERSION = "22.19.0";

/** 默认 platform / arch（前端按 UA 简化判断，Rust 端有最终决定权）。 */
export function detectPlatformArch(): { platform: string; arch: string } {
  const ua = navigator.userAgent.toLowerCase();
  const arch =
    ua.includes("arm64") || ua.includes("aarch64") ? "arm64" : "x64";
  if (ua.includes("windows")) return { platform: "win", arch };
  if (ua.includes("mac")) return { platform: "darwin", arch };
  return { platform: "linux", arch };
}

/**
 * 全局 launcher 状态。前端启动时 `fetchStatus()`，根据返回的 `phase`：
 * - `first_run` → 渲染 FirstRun 向导（M2）
 * - `idle` → 自动调 `startHost()`，成功后 `ready`
 * - 任何步骤失败 → `error`，持有 `error` payload 给 ErrorDialog 展示
 */
export const useLauncherStore = defineStore("launcher", () => {
  /** 当前 phase。初始 `booting`，调 `fetchStatus` 后切换。 */
  const phase = ref<LauncherPhase>("booting");

  /** Host 就绪后的 origin URL。`ready` phase 下非空。 */
  const origin = ref<string | null>(null);

  /** 当前 dsh 版本（从 `launcher_status` 快照同步）。 */
  const dshVersion = ref<string | null>(null);

  /** 当前 Node 版本。 */
  const nodeVersion = ref<string | null>(null);

  /** 错误详情。`error` phase 下非空。`resetError()` 清空。 */
  const error = shallowRef<LauncherErrorPayload | null>(null);

  /** `startHost` 是否在进行中。用于禁用按钮。 */
  const starting = ref(false);

  /** `installDsh` 是否在进行中。用于禁用按钮。 */
  const installingDsh = ref(false);

  /** `shutdownHost` 是否在进行中。 */
  const stopping = ref(false);

  // ─── 崩溃恢复（PR-017，设计 §5.5） ───

  /** 达到崩溃重试上限（或自动重启失败）的 payload。非空时 CrashDialog 弹出。 */
  const crashLimit = shallowRef<CrashLimitPayload | null>(null);

  /** 崩溃弹窗"重试 / 回滚"操作是否进行中。 */
  const crashRecovering = ref(false);

  /** 自动重启后短暂显示"已自动恢复"提示。 */
  const autoRestartedAttempt = ref<number | null>(null);

  // ─── 错误上下文（page-flow-analysis.md §3.4） ───

  /** 上一次失败的操作。ErrorDialog 的"重试"按钮根据此值决定调哪个 action。 */
  type LastAction = "installNode" | "installDsh" | "startHost" | "shutdownHost" | null;
  const lastFailedAction = ref<LastAction>(null);

  /** 错误发生前的 phase。`resetError` 时恢复到此值。 */
  const preErrorPhase = ref<LauncherPhase | null>(null);

  /** 错误发生前的 wizardStep。`resetError` 时恢复到此值。 */
  const preErrorWizardStep = ref<WizardStep | null>(null);

  // ─── 首启向导子状态（PR-011） ───

  /** 向导步骤。`first_run` phase 下流转。 */
  const wizardStep = ref<WizardStep>("mirror_select");

  /** 内置镜像源列表。`loadMirrors()` 加载。 */
  const mirrors = ref<MirrorInfo[]>([]);

  /** 用户选中的镜像源 id。 */
  const selectedMirrorId = ref<string | null>(null);

  /** 用户输入的自定义源 URL（为空表示未使用自定义源）。 */
  const customMirrorUrl = ref<string>("");

  /** 自定义源校验结果：`null` 未校验，`MirrorInfo` 校验通过，`string` 错误信息。 */
  const customMirrorValidation = ref<MirrorInfo | string | null>(null);

  /** 当前下载进度（bytes / total）。 */
  const downloadProgress = ref<{ bytes: number; total: number | null }>({
    bytes: 0,
    total: null,
  });

  /** 下载进度单调高水位（防止事件乱序导致进度回退/抖动）。 */
  const downloadPercentHighWater = ref(0);

  /** 解压进度（0~1，由 extract start/complete 事件驱动）。 */
  const extractProgress = ref<number>(0);

  /** 安装过程中是否可用按钮（防止重复触发）。 */
  const installing = ref(false);

  /** 选中镜像源的 MirrorInfo（计算属性）。 */
  const selectedMirror = computed<MirrorInfo | null>(() => {
    if (selectedMirrorId.value === null) return null;
    // 自定义源
    if (
      customMirrorUrl.value &&
      customMirrorValidation.value &&
      typeof customMirrorValidation.value === "object"
    ) {
      return customMirrorValidation.value;
    }
    return mirrors.value.find((m) => m.id === selectedMirrorId.value) ?? null;
  });

  /** 下载进度百分比（0~100），未知 total 时为 0。
   *  使用单调递增保护，避免后端事件乱序导致进度回退/抖动。 */
  const downloadPercent = computed(() => {
    const { bytes, total } = downloadProgress.value;
    if (total === null || total === 0) return 0;
    const pct = Math.min(100, Math.round((bytes / total) * 100));
    // 防止 progress 抖动：保证百分比只增不减（除非被 reset）
    if (pct < downloadPercentHighWater.value) {
      return downloadPercentHighWater.value;
    }
    downloadPercentHighWater.value = pct;
    return pct;
  });

  /** ErrorDialog 显示时（phase=error），背景渲染哪个 phase 的视图。
   *
   * - preErrorPhase 为 booting（启动初始 fetchStatus 失败）→ 显示 idle（默认视图）
   * - preErrorPhase 为 first_run/idle/ready → 显示对应视图
   */
  const displayPhase = computed<LauncherPhase>(() => {
    if (phase.value === "error" && preErrorPhase.value) {
      // booting 时失败，用户没看到任何视图，显示 idle 作为默认背景
      return preErrorPhase.value === "booting" ? "idle" : preErrorPhase.value;
    }
    return phase.value;
  });

  /** ErrorDialog 显示时，FirstRun 渲染哪个 wizardStep 的视图。 */
  const displayWizardStep = computed<WizardStep>(() => {
    if (phase.value === "error" && preErrorWizardStep.value) {
      return preErrorWizardStep.value;
    }
    return wizardStep.value;
  });

  /** 调 `launcher_status` 同步状态。App.vue 挂载时调一次。 */
  async function refreshStatus(): Promise<void> {
    try {
      const snap = await fetchStatus();
      applySnapshot(snap);
    } catch (e) {
      fail(e);
    }
  }

  /** 把 `StatusSnapshot` 应用到 store 状态。 */
  function applySnapshot(snap: StatusSnapshot): void {
    dshVersion.value = snap.dsh_version;
    nodeVersion.value = snap.node_version;
    // 后端 host_origin 恒为 null（不持久化）。ready 状态下保留现有 origin，
    // 避免崩溃恢复流程末尾的 refreshStatus 清掉正在使用的 origin。
    if (!(phase.value === "ready" && snap.host_origin === null)) {
      origin.value = snap.host_origin;
    }
    // Rust 端 M1 阶段不返回 `ready`（host_origin 不持久化），这里仍做防御性映射。
    if (snap.phase === "ready" && snap.host_origin) {
      phase.value = "ready";
      return;
    }
    if (snap.phase === "first_run") {
      // 按后端 §3.6 规则派生 wizardStep：
      // - node_version 有值 → done（Node 已装，显示"安装 dsh"按钮）
      // - node_version 无值 → mirror_select（从头开始）
      // 但不覆盖进行中的 downloading/extracting（installNode 自己设 wizardStep）
      if (
        phase.value !== "first_run" ||
        wizardStep.value === "mirror_select"
      ) {
        wizardStep.value = snap.node_version ? "done" : "mirror_select";
      }
      phase.value = "first_run";
      return;
    }
    // snap.phase === "idle"
    // 后端 `launcher_status` 不感知 Host 存活（host_origin 不持久化），
    // ready 状态（Host 已由 restartHost/startHost 启动）不被降级为 idle，
    // 否则崩溃恢复流程 rollbackAfterCrash 末尾的 refreshStatus 会把 ready 打回 idle。
    if (phase.value !== "first_run" && phase.value !== "ready") {
      phase.value = "idle";
    }
  }

  /** 启动 Host。成功后 phase → `ready`。 */
  async function startHostAction(): Promise<void> {
    if (starting.value) return;
    starting.value = true;
    try {
      const o = await startHost();
      origin.value = o;
      phase.value = "ready";
      error.value = null;
      lastFailedAction.value = null;
    } catch (e) {
      fail(e, "startHost");
    } finally {
      starting.value = false;
    }
  }

  /** 关闭 Host。幂等。成功后 phase → `idle`。 */
  async function shutdownHostAction(): Promise<void> {
    if (stopping.value) return;
    stopping.value = true;
    try {
      await shutdownHost();
      origin.value = null;
      if (phase.value === "ready") {
        phase.value = "idle";
      }
    } catch (e) {
      fail(e, "shutdownHost");
    } finally {
      stopping.value = false;
    }
  }

  /** 安装 dsh：拉 registry 元数据 → npm install → promote。
   *  成功后刷新 status，phase 切到 idle（用户可点"启动 Host"）。 */
  async function installDshAction(): Promise<void> {
    if (installingDsh.value) return;
    installingDsh.value = true;
    try {
      await installDsh();
      await refreshStatus();
      lastFailedAction.value = null;
    } catch (e) {
      fail(e, "installDsh");
    } finally {
      installingDsh.value = false;
    }
  }

  // ─── 崩溃恢复 actions（PR-017，设计 §5.5） ───

  /** 崩溃弹窗"重试"：清零计数器后重启 Host。成功 → phase=ready + 更新 origin。 */
  async function retryAfterCrash(): Promise<void> {
    if (crashRecovering.value) return;
    crashRecovering.value = true;
    try {
      const o = await restartHost();
      origin.value = o;
      phase.value = "ready";
      crashLimit.value = null;
      error.value = null;
    } catch (e) {
      fail(e, "startHost");
    } finally {
      crashRecovering.value = false;
    }
  }

  /** 崩溃弹窗"回滚"：切到 known_good 后重启。成功 → phase=ready。 */
  async function rollbackAfterCrash(): Promise<void> {
    if (crashRecovering.value) return;
    crashRecovering.value = true;
    try {
      await rollbackDsh();
      const o = await restartHost();
      origin.value = o;
      phase.value = "ready";
      crashLimit.value = null;
      error.value = null;
      await refreshStatus();
    } catch (e) {
      fail(e, "startHost");
    } finally {
      crashRecovering.value = false;
    }
  }

  /** 崩溃弹窗"忽略"：不重启，回到 idle 视图。 */
  function dismissCrash(): void {
    crashLimit.value = null;
    if (phase.value === "ready") {
      phase.value = "idle";
      origin.value = null;
    }
  }

  /**
   * 初始化崩溃恢复事件监听（PR-017）。App.vue 挂载时调用一次。
   *
   * - `host-crash-limit`：达到重试上限/自动重启失败 → 弹 CrashDialog
   * - `host-restarted`：自动重启成功 → 更新 origin（iframe 自动跟随），短暂提示
   */
  async function initCrashEvents(): Promise<void> {
    const { listen } = await import("@tauri-apps/api/event");

    await listen<CrashLimitPayload>("host-crash-limit", (ev) => {
      crashLimit.value = ev.payload;
      // Host 已死：origin 失效。若正处于 ready，先保持视图，弹窗覆盖其上。
    });

    await listen<HostRestartedPayload>("host-restarted", (ev) => {
      origin.value = ev.payload.origin;
      phase.value = "ready";
      crashLimit.value = null;
      autoRestartedAttempt.value = ev.payload.attempt;
      // 5 秒后清除"已自动恢复"提示
      setTimeout(() => {
        autoRestartedAttempt.value = null;
      }, 5000);
    });
  }

  /** 清除错误，决定下一步 phase。
   *
   * - 未装 Node/dsh（kind=node_not_installed / dsh_not_installed）→ 切回 `first_run`
   *   并根据 `nodeVersion` 选 `wizardStep`：Node 已装 → done，否则 → mirror_select。
   * - 其他错误 → 恢复到错误前 `phase` / `wizardStep`（`preErrorPhase` / `preErrorWizardStep`）。
   */
  function resetError(): void {
    if (error.value === null) return;
    const kind = error.value?.kind;
    error.value = null;

    // Node/dsh 未装：强制 first_run，根据 nodeVersion 选 wizardStep
    if (kind === "node_not_installed" || kind === "dsh_not_installed") {
      phase.value = "first_run";
      wizardStep.value = nodeVersion.value ? "done" : "mirror_select";
      return;
    }

    // 其他错误：恢复到错误前 phase / wizardStep
    if (preErrorPhase.value) {
      phase.value = preErrorPhase.value;
    }
    if (preErrorWizardStep.value) {
      wizardStep.value = preErrorWizardStep.value;
    }
  }

  /** 重试上一次失败的操作。ErrorDialog 的"重试"按钮调用。 */
  async function retryLastAction(): Promise<void> {
    const action = lastFailedAction.value;
    error.value = null;

    // 恢复到错误前 phase / wizardStep
    if (preErrorPhase.value) {
      phase.value = preErrorPhase.value;
    }
    if (preErrorWizardStep.value) {
      wizardStep.value = preErrorWizardStep.value;
    }

    switch (action) {
      case "installNode":
        await installNodeAction();
        break;
      case "installDsh":
        await installDshAction();
        break;
      case "startHost":
        await startHostAction();
        break;
      case "shutdownHost":
        await shutdownHostAction();
        break;
      case null:
        // 没有上次操作，只恢复 phase
        break;
    }
  }

  // ─── 首启向导 actions（PR-011） ───

  /** 加载内置镜像源列表。向导挂载时调一次。 */
  async function loadMirrors(): Promise<void> {
    try {
      mirrors.value = await listMirrors();
      // 默认选中第一个内置源（npmmirror，国内更快）
      if (selectedMirrorId.value === null && mirrors.value.length > 0) {
        selectedMirrorId.value = mirrors.value[0]!.id;
      }
    } catch (e) {
      fail(e);
    }
  }

  /** 探活镜像源，自动选中首个可用源。失败不切到 failed，只切 phase=error。 */
  async function autoPickMirror(): Promise<void> {
    const prevStep = wizardStep.value;
    wizardStep.value = "probing";
    try {
      const custom = customMirrorUrl.value ? [customMirrorUrl.value] : undefined;
      const picked = await probeMirrors(custom);
      selectedMirrorId.value = picked.id;
      // 如果是自定义源，同步到 mirrors 列表
      if (!mirrors.value.some((m) => m.id === picked.id)) {
        mirrors.value = [...mirrors.value, picked];
      }
      wizardStep.value = prevStep === "probing" ? "mirror_select" : prevStep;
    } catch (e) {
      // 探活失败不致命，回到 mirror_select 让用户手动选
      wizardStep.value = "mirror_select";
      // 只切 phase=error，不动 wizardStep（避免被 fail 改成 failed）
      if (
        typeof e === "object" &&
        e !== null &&
        "kind" in e &&
        "message" in e
      ) {
        error.value = e as LauncherErrorPayload;
      } else {
        error.value = {
          kind: "io",
          message: e instanceof Error ? e.message : String(e),
        };
      }
      phase.value = "error";
    }
  }

  /** 校验自定义源 URL。用户输入时 debounce 调用。 */
  async function validateCustomMirrorAction(url: string): Promise<void> {
    if (!url) {
      customMirrorValidation.value = null;
      return;
    }
    try {
      const m = await validateCustomMirror(url);
      customMirrorValidation.value = m;
    } catch (e) {
      // LauncherErrorPayload 形如 { kind, message }，提取 message
      if (
        typeof e === "object" &&
        e !== null &&
        "message" in e &&
        typeof (e as { message: unknown }).message === "string"
      ) {
        customMirrorValidation.value = (e as { message: string }).message;
      } else {
        customMirrorValidation.value = e instanceof Error ? e.message : String(e);
      }
    }
  }

  /** 选择镜像源。`id` 为内置源 id 或 `custom:<url>`。 */
  function selectMirror(id: string): void {
    selectedMirrorId.value = id;
  }

  /**
   * 触发 Node 安装：下载 + 校验 + 解压 + 写 state。
   * 成功后向导 → `done`，并刷新状态（applySnapshot 会保持 `first_run` phase，
   * 让 FirstRun 的"启动 dsh"按钮可见，避免切到 idle 视图循环报错）。
   */
  async function installNodeAction(opts?: {
    version?: string;
  }): Promise<void> {
    if (installing.value) return;
    installing.value = true;
    wizardStep.value = "downloading";
    downloadProgress.value = { bytes: 0, total: null };
    downloadPercentHighWater.value = 0;
    extractProgress.value = 0;

    try {
      const version = opts?.version ?? DEFAULT_NODE_VERSION;
      const mirror = selectedMirror.value;
      if (!mirror) {
        throw new Error("未选择镜像源");
      }

      const { platform, arch } = detectPlatformArch();
      await installNode({
        version,
        mirrorBaseUrl: mirror.base_url,
        platform,
        arch,
      });

      wizardStep.value = "done";
      // 刷新状态：state.json 已写入，但 applySnapshot 会保持 first_run phase，
      // 让用户在 FirstRun 的 done 步骤点击"启动 dsh"。
      await refreshStatus();
    } catch (e) {
      // 重置向导到 mirror_select，让用户重新选源或直接重试
      wizardStep.value = "mirror_select";
      downloadProgress.value = { bytes: 0, total: null };
      downloadPercentHighWater.value = 0;
      extractProgress.value = 0;
      fail(e, "installNode");
    } finally {
      installing.value = false;
    }
  }

  /** 应用进度事件。由 Tauri event listener 调用。 */
  function applyProgressEvent(ev: ProgressEvent): void {
    if (ev.stage === "download") {
      downloadProgress.value = { bytes: ev.bytes, total: ev.total };
    } else if (ev.stage === "extract") {
      // Rust 端 emit 两次：start (total=None) + complete (total=Some(0))
      // 用 total === 0（不是 null）判断 complete
      if (ev.total === 0) {
        extractProgress.value = 1;
      } else {
        wizardStep.value = "extracting";
        extractProgress.value = 0.5;
      }
    }
  }

  /** 重置向导到初始状态（允许重试）。 */
  function resetWizard(): void {
    wizardStep.value = "mirror_select";
    downloadProgress.value = { bytes: 0, total: null };
    downloadPercentHighWater.value = 0;
    extractProgress.value = 0;
    error.value = null;
  }

  /** 把异常转成 `LauncherErrorPayload` 并切到 `error` phase。
   *
   * @param action 失败的操作类型，用于 ErrorDialog 的"重试"按钮。null 表示非关键操作（如 loadMirrors）。
   */
  function fail(e: unknown, action: LastAction = null): void {
    // 记录错误上下文（page-flow-analysis.md §3.4）
    if (action !== null) {
      lastFailedAction.value = action;
    }
    preErrorPhase.value = phase.value;
    preErrorWizardStep.value = wizardStep.value;

    if (
      typeof e === "object" &&
      e !== null &&
      "kind" in e &&
      "message" in e
    ) {
      error.value = e as LauncherErrorPayload;
    } else {
      error.value = {
        kind: "io",
        message: e instanceof Error ? e.message : String(e),
      };
    }

    phase.value = "error";
  }

  return {
    // 基础状态
    phase,
    origin,
    dshVersion,
    nodeVersion,
    error,
    starting,
    stopping,
    installingDsh,
    // 崩溃恢复（PR-017）
    crashLimit,
    crashRecovering,
    autoRestartedAttempt,
    retryAfterCrash,
    rollbackAfterCrash,
    dismissCrash,
    initCrashEvents,
    // 错误上下文
    lastFailedAction,
    preErrorPhase,
    preErrorWizardStep,
    displayPhase,
    displayWizardStep,
    // 向导状态
    wizardStep,
    mirrors,
    selectedMirrorId,
    customMirrorUrl,
    customMirrorValidation,
    downloadProgress,
    extractProgress,
    installing,
    // 计算属性
    selectedMirror,
    downloadPercent,
    // actions
    refreshStatus,
    startHost: startHostAction,
    shutdownHost: shutdownHostAction,
    resetError,
    retryLastAction,
    loadMirrors,
    autoPickMirror,
    validateCustomMirror: validateCustomMirrorAction,
    selectMirror,
    installNode: installNodeAction,
    installDsh: installDshAction,
    applyProgressEvent,
    resetWizard,
  };
});
