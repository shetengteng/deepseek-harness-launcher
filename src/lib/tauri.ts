// Tauri invoke 封装。对应设计 §M1.5。
// 所有命令调用都走这里，统一错误类型 `LauncherError`（Rust 端 `SerializableError`）。

import { invoke } from "@tauri-apps/api/core";

/** Rust 端 `LauncherError` 序列化结构（`error.rs::SerializableError`）。 */
export interface LauncherErrorPayload {
  /** 错误类型 tag，对应 `LauncherError::kind_str`：`state_corrupt` / `io` / `host` / `path_resolve` 等 */
  kind: string;
  /** 人类可读错误信息（`thiserror::Error` 的 `Display`） */
  message: string;
  /** 用户可见的中文文案 + 可操作提示（PR-019）。ErrorDialog 优先展示此字段。 */
  user_message?: string;
  /** 可选的结构化数据（如 `state_corrupt` 的 `path`、`unsupported_schema_version` 的 `version`） */
  data?: Record<string, unknown>;
}

/** `launcher_status` 返回的快照（`commands.rs::StatusSnapshot`）。 */
export interface StatusSnapshot {
  /** `first_run`：无 state.json；`idle`：state 存在但 Host 未启动；`ready`：Host 已启动（M1 阶段不持久化，前端拿到 idle 后自行 start） */
  phase: "first_run" | "idle" | "ready";
  /** Host 就绪后的 origin URL，前端 webview/iframe 加载此 URL。M1 阶段恒为 null。 */
  host_origin: string | null;
  /** 当前 dsh 版本（`state.dsh.current`）。 */
  dsh_version: string | null;
  /** 当前 Node 版本（`state.node.version`）。 */
  node_version: string | null;
  /** 宿主平台（`darwin` / `win` / `linux`），Rust `std::env::consts` 归一化。 */
  platform: string;
  /** 宿主架构（`arm64` / `x64`）。WKWebView UA 误报 Intel，以此为准。 */
  arch: string;
}

/**
 * 调用 Tauri 命令并把 Rust 端的反序列化错误转成 `LauncherErrorPayload`。
 *
 * Tauri 在命令返回 `Err(LauncherError)` 时，会把 `LauncherError` 的 `Serialize` 输出作为 rejection payload。
 * 前端拿到的是 `{ kind, message, data }` 三段式结构。
 */
export async function invokeCommand<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  return invoke<T>(command, args).catch((err: unknown): Promise<T> => {
    // Rust 端返回的 `LauncherError` 序列化后形如 `{ kind, message, data }`。
    // 其他情况（如命令未注册、参数序列化失败）包装成通用 `io` 错误。
    if (
      typeof err === "object" &&
      err !== null &&
      "kind" in err &&
      "message" in err
    ) {
      throw err as LauncherErrorPayload;
    }
    const payload: LauncherErrorPayload = {
      kind: "io",
      message: err instanceof Error ? err.message : String(err),
    };
    throw payload;
  });
}

/** 调 `launcher_status`。 */
export function fetchStatus(): Promise<StatusSnapshot> {
  return invokeCommand<StatusSnapshot>("launcher_status");
}

/** 调 `start_host`，返回 origin URL。 */
export function startHost(): Promise<string> {
  return invokeCommand<string>("start_host");
}

/** 调 `shutdown_host`，幂等。 */
export function shutdownHost(): Promise<void> {
  return invokeCommand<void>("shutdown_host");
}

// ─── PR-017: 崩溃恢复 ───

/** `host-crash-limit` 事件 payload（`commands.rs::CrashLimitPayload`）。 */
export interface CrashLimitPayload {
  crash_counter: number;
  retry_limit: number;
  exit_code: number | null;
  exit_signal: number | null;
  known_good: string | null;
}

/** `host-restarted` 事件 payload（`commands.rs::HostRestartedPayload`）。 */
export interface HostRestartedPayload {
  attempt: number;
  origin: string;
}

/** 调 `restart_host`：清零崩溃计数后重启 Host（崩溃弹窗"重试"按钮）。 */
export function restartHost(): Promise<string> {
  return invokeCommand<string>("restart_host");
}

/** 调 `rollback_dsh_command`：回滚到 known_good（崩溃弹窗"回滚"按钮）。 */
export function rollbackDsh(): Promise<string> {
  return invokeCommand<string>("rollback_dsh_command");
}

// ─── PR-011: 首启向导镜像源 + Node 安装 ───

/** 镜像源信息。对应 Rust `MirrorInfo`。 */
export interface MirrorInfo {
  id: string;
  name: string;
  base_url: string;
  trusted: boolean;
}

/** 下载进度事件 payload。对应 Rust `ProgressEvent`。 */
export interface ProgressEvent {
  /** `download` / `extract` */
  stage: string;
  /** 已传输字节数（extract 阶段为 0） */
  bytes: number;
  /** 总字节数（未知为 null） */
  total: number | null;
}

/** dsh 安装阶段事件。对应 Rust `DshInstallProgressPayload`。 */
export interface DshInstallProgressEvent {
  stage: "resolving" | "downloading" | "installing" | "verifying";
}

/** 调 `list_mirrors`：返回内置镜像源。 */
export function listMirrors(): Promise<MirrorInfo[]> {
  return invokeCommand<MirrorInfo[]>("list_mirrors");
}

/** 调 `probe_mirrors_command`：探活镜像源，返回首个可用源。 */
export function probeMirrors(customUrls?: string[]): Promise<MirrorInfo> {
  return invokeCommand<MirrorInfo>("probe_mirrors_command", {
    customUrls: customUrls ?? null,
  });
}

/** 调 `validate_custom_mirror_command`：校验自定义源 URL。 */
export function validateCustomMirror(url: string): Promise<MirrorInfo> {
  return invokeCommand<MirrorInfo>("validate_custom_mirror_command", { url });
}

/** 首启时冻结的 dsh 与 Node 安装计划。 */
export interface BootstrapPlan {
  dsh_version: string;
  registry: string;
  engines_node: string | null;
  node_version: string;
  requirement_source: "dsh-engines" | "launcher-verified-fallback";
  resolved_at: string;
  phase: string;
}

/** 当前 registry 中的 dsh `latest` 版本。 */
export interface LatestDshVersion {
  latest_version: string;
}

/** 查询当前可更新到的 dsh 版本。 */
export function getLatestDshVersion(): Promise<LatestDshVersion> {
  return invokeCommand<LatestDshVersion>("get_latest_dsh_version_command");
}

/** 冻结当前 `latest` 对应的 dsh 与 Node 安装计划。 */
export function resolveBootstrapPlan(): Promise<BootstrapPlan> {
  return invokeCommand<BootstrapPlan>("resolve_bootstrap_plan_command");
}

/** `install_node_command` 参数。 */
export interface InstallNodeArgs {
  version: string;
  mirrorBaseUrl: string;
  platform: string;
  arch: string;
}

/** 调 `install_node_command`：下载 + 校验 + 解压 + 写 state。 */
export function installNode(args: InstallNodeArgs): Promise<string> {
  // Rust 端用 snake_case，前端转一下
  return invokeCommand<string>("install_node_command", {
    args: {
      version: args.version,
      mirror_base_url: args.mirrorBaseUrl,
      platform: args.platform,
      arch: args.arch,
    },
  });
}

/** 安装冻结版本或 registry 当前的 `latest`。
 * `deferActivation` 为 true 时只设为 pending，等待用户重启后试运行。 */
export function installDsh(deferActivation = false): Promise<string> {
  return invokeCommand<string>("install_dsh_command", { deferActivation });
}

// ─── 设置页状态与来源配置 ───

/** dsh 状态详情，供设置页展示。对应 Rust `DshStateSnapshot`。 */
export interface DshStateSnapshot {
  current: string | null;
  known_good: string | null;
  pending: string | null;
  node_mirror: string;
  registry: string;
  installed: InstalledDshInfo[];
}

export interface InstalledDshInfo {
  version: string;
  installed_at: string;
  status: string;
}

/** 调 `get_dsh_state`：返回 dsh 状态详情。 */
export function getDshState(): Promise<DshStateSnapshot> {
  return invokeCommand<DshStateSnapshot>("get_dsh_state");
}

/** 更新后续 Node.js 下载使用的来源。 */
export function setNodeMirror(mirror: string): Promise<void> {
  return invokeCommand<void>("set_node_mirror_command", { mirror });
}

/** 更新后续 dsh 安装和更新使用的 npm 下载源。 */
export function setRegistry(registry: string): Promise<void> {
  return invokeCommand<void>("set_registry_command", { registry });
}

// ─── PR-019: 诊断导出 ───

/** 调 `export_diagnostics`：打包 state.json + 日志为 zip。返回写入字节数。 */
export function exportDiagnostics(dest: string): Promise<number> {
  return invokeCommand<number>("export_diagnostics", { dest });
}

/** 调 `uninstall_managed_runtime`：移除应用托管的 dsh、Node 和设置后退出应用。 */
export function uninstallManagedRuntime(): Promise<void> {
  return invokeCommand<void>("uninstall_managed_runtime");
}
