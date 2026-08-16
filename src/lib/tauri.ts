// Tauri invoke 封装。对应设计 §M1.5。
// 所有命令调用都走这里，统一错误类型 `LauncherError`（Rust 端 `SerializableError`）。

import { invoke } from "@tauri-apps/api/core";

/** Rust 端 `LauncherError` 序列化结构（`error.rs::SerializableError`）。 */
export interface LauncherErrorPayload {
  /** 错误类型 tag，对应 `LauncherError::kind_str`：`state_corrupt` / `io` / `host` / `path_resolve` 等 */
  kind: string;
  /** 人类可读错误信息（`thiserror::Error` 的 `Display`） */
  message: string;
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

/** 调 `install_dsh_command`：拉 registry → npm install → 校验 → promote_to_current。
 *  返回安装的 dsh 版本号。无参数：从 state 读 registry，从 dist-tags 拿 latest。 */
export function installDsh(): Promise<string> {
  return invokeCommand<string>("install_dsh_command");
}

// ─── PR-015/PR-016: 升级编排 + 设置页 ───

/** dsh 状态详情，供设置页展示。对应 Rust `DshStateSnapshot`。 */
export interface DshStateSnapshot {
  current: string | null;
  known_good: string | null;
  pending: string | null;
  pinned_range: string;
  auto_upgrade: boolean;
  check_interval_hours: number;
  registry: string;
  installed: InstalledDshInfo[];
  ignored_versions: string[];
}

export interface InstalledDshInfo {
  version: string;
  installed_at: string;
  status: string;
}

/** 升级检查结果。对应 Rust `UpgradeCheckResult`。 */
export interface UpgradeCheckResult {
  available: boolean;
  version: string | null;
  engines_node: string | null;
}

/** 调 `get_dsh_state`：返回 dsh 状态详情。 */
export function getDshState(): Promise<DshStateSnapshot> {
  return invokeCommand<DshStateSnapshot>("get_dsh_state");
}

/** 调 `check_for_upgrade_command`：检查 registry 是否有可升级版本。 */
export function checkForUpgrade(): Promise<UpgradeCheckResult> {
  return invokeCommand<UpgradeCheckResult>("check_for_upgrade_command");
}

/** 调 `prepare_upgrade_command`：下载安装新版本，设 pending。 */
export function prepareUpgrade(): Promise<string> {
  return invokeCommand<string>("prepare_upgrade_command");
}

// ─── PR-016: 设置管理命令 ───

/** 更新 pinned_range。 */
export function setPinnedRange(range: string): Promise<void> {
  return invokeCommand<void>("set_pinned_range_command", { range });
}

/** 切换 auto_upgrade。 */
export function setAutoUpgrade(enabled: boolean): Promise<void> {
  return invokeCommand<void>("set_auto_upgrade_command", { enabled });
}

/** 更新检查间隔。 */
export function setCheckInterval(hours: number): Promise<void> {
  return invokeCommand<void>("set_check_interval_command", { hours });
}

/** 忽略指定版本。 */
export function ignoreVersion(version: string): Promise<void> {
  return invokeCommand<void>("ignore_version_command", { version });
}

/** 取消忽略指定版本。 */
export function unignoreVersion(version: string): Promise<void> {
  return invokeCommand<void>("unignore_version_command", { version });
}
