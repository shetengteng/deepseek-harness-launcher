// Tauri invoke 封装。对应设计 §M1.5。
// 所有命令调用都走这里，统一错误类型 `LauncherError`（Rust 端 `SerializableError`）。

import { invoke } from "@tauri-apps/api/core";

/** `LauncherError` 序列化结构（`error.rs::SerializableError`）。 */
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

/** dsh 更新因 Node 不兼容而需要用户确认升级。 */
export interface NodeUpgradeRequired {
  dsh_version: string;
  current_node: string;
  engines_node: string;
  suggested_node: string;
}

export function parseNodeUpgradeRequired(
  error: unknown,
): NodeUpgradeRequired | null {
  if (typeof error !== "object" || error === null || !("kind" in error)) {
    return null;
  }
  const payload = error as LauncherErrorPayload;
  if (payload.kind !== "node_upgrade_required" || !payload.data) return null;
  const dshVersion = String(payload.data.dsh_version ?? "");
  const currentNode = String(payload.data.current_node ?? "");
  const enginesNode = String(payload.data.engines_node ?? "");
  const suggestedNode = String(payload.data.suggested_node ?? "");
  if (!dshVersion || !currentNode || !enginesNode || !suggestedNode)
    return null;
  return {
    dsh_version: dshVersion,
    current_node: currentNode,
    engines_node: enginesNode,
    suggested_node: suggestedNode,
  };
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

export interface AboutInfo {
  launcher_version: string;
  dsh_version: string | null;
  node_version: string | null;
  data_directory: string;
}

/** 调 `get_about_info`：返回启动器和托管运行时信息。 */
export function getAboutInfo(): Promise<AboutInfo> {
  return invokeCommand<AboutInfo>("get_about_info");
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

export interface DshUpgradeRestartResult {
  origin: string;
  active_version: string;
  rolled_back: boolean;
}

export function restartHostAfterDshUpdate(): Promise<DshUpgradeRestartResult> {
  return invokeCommand<DshUpgradeRestartResult>(
    "restart_host_after_dsh_update",
  );
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

/** 启动后的轻量检查结果。`null` 表示没有新版本或该版本已经提示过。 */
export interface DshUpdateInfo {
  current_version: string;
  latest_version: string;
}

/** 查询当前可更新到的 dsh 版本。 */
export function getLatestDshVersion(): Promise<LatestDshVersion> {
  return invokeCommand<LatestDshVersion>("get_latest_dsh_version_command");
}

/** 轻量检查一次 dsh 更新，并记录已提示的版本。 */
export function checkDshUpdate(): Promise<DshUpdateInfo | null> {
  return invokeCommand<DshUpdateInfo | null>("check_dsh_update_command");
}

/** 冻结当前 `latest` 对应的 dsh 与 Node 安装计划。 */
export function resolveBootstrapPlan(): Promise<BootstrapPlan> {
  return invokeCommand<BootstrapPlan>("resolve_bootstrap_plan_command");
}

/** `install_node_command` 参数。 */
export interface InstallNodeArgs {
  version: string;
  operationId: string;
  mirrorBaseUrl: string;
  platform: string;
  arch: string;
}

/** 调 `install_node_command`：下载 + 校验 + 解压 + 写 state。 */
export function installNode(args: InstallNodeArgs): Promise<string> {
  return invokeCommand<string>("install_node_command", {
    args: {
      version: args.version,
      operation_id: args.operationId,
      mirror_base_url: args.mirrorBaseUrl,
      platform: args.platform,
      arch: args.arch,
    },
  });
}

/** 更新场景下按当前镜像源安装目标 Node，并原子切换 VERSION。 */
export function upgradeNode(options: {
  version: string;
  operationId: string;
}): Promise<string> {
  return invokeCommand<string>("upgrade_node_command", {
    version: options.version,
    operationId: options.operationId,
  });
}

export interface NodeUpdateTarget {
  current_version: string;
  target_version: string;
  engines_node: string | null;
  target_source: "dsh-engines" | "launcher-verified-fallback";
  update_available: boolean;
}

/** 查询当前 dsh 支持的最新 Node.js 版本，供手动更新前确认。 */
export function getNodeUpdateTarget(): Promise<NodeUpdateTarget> {
  return invokeCommand<NodeUpdateTarget>("get_node_update_target_command");
}

/** 取消当前 Node.js 安装任务。 */
export function cancelNodeInstall(operationId: string): Promise<boolean> {
  return invokeCommand<boolean>("cancel_node_install_command", {
    operationId,
  });
}

/** 安装首启冻结版本，或用户已经确认的精确更新版本。 */
export function installDsh(options?: {
  operationId?: string;
  expectedVersion?: string;
}): Promise<string> {
  return invokeCommand<string>("install_dsh_command", {
    operationId: options?.operationId ?? null,
    expectedVersion: options?.expectedVersion ?? null,
  });
}

/** 取消当前 dsh 安装任务。 */
export function cancelDshInstall(operationId: string): Promise<boolean> {
  return invokeCommand<boolean>("cancel_dsh_install_command", {
    operationId,
  });
}

// ─── 设置页状态与来源配置 ───

export type ThemeMode = "light" | "dark";

/** 读取启动器主题偏好；dsh iframe 不读取也不会收到此值。 */
export function getTheme(): Promise<ThemeMode> {
  return invokeCommand<ThemeMode>("get_theme_command");
}

/** 保存启动器主题偏好；只影响 launcher 自身的根页面。 */
export function setTheme(theme: ThemeMode): Promise<void> {
  return invokeCommand<void>("set_theme_command", { theme });
}

/** dsh 状态详情，供设置页展示。对应 Rust `DshStateSnapshot`。 */
export interface DshStateSnapshot {
  current: string | null;
  known_good: string | null;
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

export interface DshCliInstallResult {
  command_path: string;
  path_instruction: string;
}

/** 安装解析启动器托管运行时的稳定 `dsh` 命令。 */
export function installDshCli(): Promise<DshCliInstallResult> {
  return invokeCommand<DshCliInstallResult>("install_dsh_cli_command");
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

// ─── 插件市场 ───

export type MarketplaceSort = "relevance" | "updated" | "stars";
export type MarketplacePluginStatus =
  | "available"
  | "installed"
  | "update_available"
  | "unknown"
  | "operation_running";

export interface MarketplaceInstallSpec {
  owner: string;
  repository: string;
  subdirectory: string | null;
  reference: string | null;
  source: string;
}

export interface MarketplacePopularity {
  marketplace_rank: number | null;
  ranking_updated_at: string | null;
  github_stars: number | null;
  stars_fetched_at: string | null;
}

export interface MarketplacePlugin {
  id: string;
  name: string;
  repository_url: string | null;
  install_spec: MarketplaceInstallSpec | null;
  description: string;
  category: string | null;
  category_id: string | null;
  tags: string[];
  source_updated_at: string | null;
  validated_at: string | null;
  popularity: MarketplacePopularity;
  status: MarketplacePluginStatus;
  installation_id: string | null;
  installed_source: string | null;
  local_only: boolean;
}

export interface MarketplaceSource {
  label: string;
  url: string;
  fetched_at: string | null;
  stale: boolean;
  catalog_updated_at: string | null;
  catalog_count: number | null;
}

export interface MarketplaceSnapshot {
  source: MarketplaceSource;
  plugins: MarketplacePlugin[];
  profiles: string[];
}

export interface MarketplaceQuery {
  query?: string;
  category?: string;
  installedOnly?: boolean;
  sort: MarketplaceSort;
  profile: string;
}

export interface MarketplaceOperation {
  id: string;
  kind: "install" | "custom_install" | "remove";
  plugin_id: string;
  profile: string;
  phase: "preparing" | "running" | "verifying" | "succeeded" | "failed";
  message: string;
  log_path: string | null;
}

export interface MarketplaceOperationEvent {
  operation: MarketplaceOperation;
}

export interface MarketplaceCustomInstallPreview {
  profile: string;
  source: string;
  dsh_version: string;
}

export function marketplaceQuery(
  query: MarketplaceQuery,
): Promise<MarketplaceSnapshot> {
  return invokeCommand<MarketplaceSnapshot>("marketplace_query", { query });
}

export function marketplaceRefresh(): Promise<MarketplaceSnapshot> {
  return invokeCommand<MarketplaceSnapshot>("marketplace_refresh");
}

export function marketplaceParseCustomInstall(
  command: string,
): Promise<MarketplaceCustomInstallPreview> {
  return invokeCommand<MarketplaceCustomInstallPreview>(
    "marketplace_parse_custom_install",
    { request: { command } },
  );
}

export function marketplaceInstall(options: {
  pluginId: string;
  profile: string;
}): Promise<MarketplaceOperation> {
  return invokeCommand<MarketplaceOperation>("marketplace_install", {
    request: options,
  });
}

export function marketplaceInstallCustom(
  command: string,
): Promise<MarketplaceOperation> {
  return invokeCommand<MarketplaceOperation>("marketplace_install_custom", {
    request: { command },
  });
}

export function marketplaceRemove(options: {
  installationId: string;
  profile: string;
}): Promise<MarketplaceOperation> {
  return invokeCommand<MarketplaceOperation>("marketplace_remove", {
    request: options,
  });
}
