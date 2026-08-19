import type { MarketplacePlugin, MarketplacePluginStatus } from "@/lib/tauri";

export type MarketplacePendingAction = "install" | "remove" | null;

export function marketplaceInstallSpec(plugin: MarketplacePlugin): string {
  return plugin.install_spec?.source || plugin.installed_source || "目录未提供";
}

export function marketplaceSourceKind(
  plugin: MarketplacePlugin,
): "github" | "npm" | "source" {
  const source = marketplaceInstallSpec(plugin);
  if (source.startsWith("github:")) return "github";
  if (source.startsWith("npm:")) return "npm";
  return "source";
}

export function marketplaceRepositoryId(plugin: MarketplacePlugin): string {
  if (plugin.id.startsWith("local:")) return "本地 profile";
  return plugin.id;
}

export function marketplaceDate(value: string | null): string {
  if (!value) return "目录未提供";
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleDateString();
}

export function marketplaceStatusLabel(
  status: MarketplacePluginStatus,
): string {
  if (status === "installed") return "已安装";
  if (status === "update_available") return "有可用更新";
  if (status === "unknown") return "状态待确认";
  if (status === "operation_running") return "正在处理";
  return "可安装";
}
