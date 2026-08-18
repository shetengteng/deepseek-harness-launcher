import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { listen } from "@tauri-apps/api/event";
import {
  marketplaceInstall,
  marketplaceInstallCustom,
  marketplaceParseCustomInstall,
  marketplaceQuery,
  marketplaceRefresh,
  marketplaceRemove,
  type MarketplaceCustomInstallPreview,
  type MarketplaceOperation,
  type MarketplacePlugin,
  type MarketplaceSnapshot,
} from "@/lib/tauri";

export type MarketplaceTab = "market" | "installed";

export const useMarketplaceStore = defineStore("marketplace", () => {
  const snapshot = ref<MarketplaceSnapshot | null>(null);
  const loading = ref(false);
  const refreshing = ref(false);
  const error = ref<string | null>(null);
  const operationError = ref<string | null>(null);
  const operation = ref<MarketplaceOperation | null>(null);
  const selectedId = ref<string | null>(null);
  const profile = ref("web");
  const tab = ref<MarketplaceTab>("market");
  const search = ref("");
  const category = ref("all");
  const rankLimit = ref<"all" | "top100" | "top500">("all");
  const starMinimum = ref<"all" | "100" | "250">("all");
  const sort = ref<"relevance" | "updated" | "popularity">("popularity");
  const customCommand = ref("");
  const customPreview = ref<MarketplaceCustomInstallPreview | null>(null);
  const customError = ref<string | null>(null);
  let unlistenOperation: (() => void) | null = null;

  const profiles = computed(() => snapshot.value?.profiles ?? ["web"]);
  const selectedPlugin = computed(() =>
    snapshot.value?.plugins.find((plugin) => plugin.id === selectedId.value) ??
    null,
  );

  function applySnapshot(next: MarketplaceSnapshot): void {
    snapshot.value = next;
    if (!next.profiles.includes(profile.value)) profile.value = next.profiles[0] ?? "web";
    if (!next.plugins.some((plugin) => plugin.id === selectedId.value)) {
      selectedId.value = next.plugins[0]?.id ?? null;
    }
  }

  async function load(): Promise<void> {
    loading.value = true;
    error.value = null;
    try {
      applySnapshot(
        await marketplaceQuery({
          query: search.value || undefined,
          category: category.value === "all" ? undefined : category.value,
          installedOnly: tab.value === "installed",
          sort: sort.value,
          profile: profile.value,
        }),
      );
    } catch (reason) {
      error.value = messageOf(reason);
    } finally {
      loading.value = false;
    }
  }

  async function refresh(): Promise<void> {
    refreshing.value = true;
    error.value = null;
    try {
      await marketplaceRefresh();
      await load();
    } catch (reason) {
      error.value = messageOf(reason);
    } finally {
      refreshing.value = false;
    }
  }

  async function initializeOperationEvents(): Promise<void> {
    if (unlistenOperation) return;
    unlistenOperation = await listen<{ operation: MarketplaceOperation }>(
      "marketplace://operation",
      (event) => {
        operation.value = event.payload.operation;
      },
    );
  }

  async function previewCustomInstall(): Promise<void> {
    customError.value = null;
    customPreview.value = null;
    try {
      customPreview.value = await marketplaceParseCustomInstall(
        customCommand.value,
      );
    } catch (reason) {
      customError.value = messageOf(reason);
    }
  }

  function returnToCustomCommand(): void {
    customPreview.value = null;
    customError.value = null;
  }

  async function install(plugin: MarketplacePlugin): Promise<void> {
    operationError.value = null;
    try {
      operation.value = await marketplaceInstall({
        pluginId: plugin.id,
        profile: profile.value,
      });
      await load();
    } catch (reason) {
      operationError.value = messageOf(reason);
    }
  }

  async function installCustom(): Promise<void> {
    operationError.value = null;
    try {
      operation.value = await marketplaceInstallCustom(customCommand.value);
      customPreview.value = null;
      await load();
    } catch (reason) {
      operationError.value = messageOf(reason);
    }
  }

  async function remove(plugin: MarketplacePlugin): Promise<void> {
    if (!plugin.installation_id) return;
    operationError.value = null;
    try {
      operation.value = await marketplaceRemove({
        installationId: plugin.installation_id,
        profile: profile.value,
      });
      await load();
    } catch (reason) {
      operationError.value = messageOf(reason);
    }
  }

  return {
    snapshot,
    loading,
    refreshing,
    error,
    operationError,
    operation,
    selectedId,
    profile,
    tab,
    search,
    category,
    rankLimit,
    starMinimum,
    sort,
    customCommand,
    customPreview,
    customError,
    profiles,
    selectedPlugin,
    applySnapshot,
    load,
    refresh,
    initializeOperationEvents,
    previewCustomInstall,
    returnToCustomCommand,
    install,
    installCustom,
    remove,
  };
});

function messageOf(reason: unknown): string {
  if (typeof reason === "object" && reason !== null && "user_message" in reason) {
    return String((reason as { user_message: unknown }).user_message);
  }
  if (typeof reason === "object" && reason !== null && "message" in reason) {
    return String((reason as { message: unknown }).message);
  }
  return String(reason);
}
