import {
  computed,
  onMounted,
  onUnmounted,
  ref,
  useTemplateRef,
  watch,
} from "vue";
import { useMediaQuery } from "@vueuse/core";
import type { MarketplacePendingAction } from "@/lib/marketplace";
import type { MarketplacePlugin } from "@/lib/tauri";
import { useMarketplaceStore } from "@/stores/marketplace";

type CompactPane = "catalog" | "detail";
const SEARCH_DEBOUNCE_MS = 250;

export function useMarketplaceWorkspace() {
  const store = useMarketplaceStore();
  const catalogPane = useTemplateRef<{ focusSearch: () => void }>(
    "catalogPane",
  );
  const customExpanded = ref(false);
  const pendingAction = ref<MarketplacePendingAction>(null);
  const retryOperation = ref<(() => Promise<void>) | null>(null);
  const compactPane = ref<CompactPane>("catalog");
  const isCompact = useMediaQuery("(max-width: 899px)");

  const categories = computed(() => [
    ...new Set(
      (store.snapshot?.plugins ?? [])
        .filter((plugin) => !plugin.local_only)
        .map((plugin) => plugin.category)
        .filter((category): category is string => category !== null),
    ),
  ]);

  const filteredPlugins = computed(() => {
    const query = store.search.trim().toLocaleLowerCase();
    return (store.snapshot?.plugins ?? []).filter((plugin) => {
      const matchesTab =
        store.tab === "discover"
          ? !plugin.local_only
          : plugin.status === "installed" ||
            plugin.status === "update_available";
      const searchable = [
        plugin.name,
        plugin.id,
        plugin.description,
        plugin.category ?? "",
        ...plugin.tags,
      ]
        .join(" ")
        .toLocaleLowerCase();
      return (
        matchesTab &&
        (!query || searchable.includes(query)) &&
        (store.category === "all" || plugin.category === store.category)
      );
    });
  });

  const installedCount = computed(
    () =>
      (store.snapshot?.plugins ?? []).filter(
        (plugin) =>
          plugin.status === "installed" || plugin.status === "update_available",
      ).length,
  );

  const selectedPlugin = computed(
    () =>
      filteredPlugins.value.find((plugin) => plugin.id === store.selectedId) ??
      null,
  );

  const sourceStatus = computed(() => store.snapshot?.source ?? null);

  watch(
    filteredPlugins,
    (plugins) => {
      if (!plugins.some((plugin) => plugin.id === store.selectedId)) {
        store.selectedId = plugins[0]?.id ?? null;
      }
      pendingAction.value = null;
    },
    { immediate: true },
  );

  watch(
    () => store.profile,
    () => {
      pendingAction.value = null;
      void store.load();
    },
  );

  watch(
    () => store.search,
    (_search, _previous, onCleanup) => {
      const timer = window.setTimeout(() => {
        void store.load();
      }, SEARCH_DEBOUNCE_MS);
      onCleanup(() => window.clearTimeout(timer));
    },
  );

  watch(
    () => [store.tab, store.category, store.sort] as const,
    () => {
      pendingAction.value = null;
      compactPane.value = "catalog";
      if (store.tab === "installed") {
        customExpanded.value = false;
        store.customPreview = null;
      }
      void store.load();
    },
  );

  watch(isCompact, (compact) => {
    if (!compact) compactPane.value = "catalog";
  });

  onMounted(() => {
    void store.initializeOperationEvents();
    void store.load();
    window.addEventListener("keydown", focusSearchShortcut);
  });

  onUnmounted(() => {
    window.removeEventListener("keydown", focusSearchShortcut);
  });

  function focusSearchShortcut(event: KeyboardEvent): void {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      catalogPane.value?.focusSearch();
    }
  }

  function selectPlugin(plugin: MarketplacePlugin): void {
    store.selectedId = plugin.id;
    pendingAction.value = null;
    store.clearOperationError();
    if (isCompact.value) compactPane.value = "detail";
  }

  function startCustomPreview(): void {
    void (async () => {
      await store.previewCustomInstall();
      if (store.customPreview) {
        pendingAction.value = null;
        if (isCompact.value) compactPane.value = "detail";
      }
    })();
  }

  async function confirmInstall(plugin: MarketplacePlugin): Promise<void> {
    retryOperation.value = () => confirmInstall(plugin);
    await store.install(plugin);
    if (!store.operationError) {
      pendingAction.value = null;
      retryOperation.value = null;
    }
  }

  async function confirmRemove(plugin: MarketplacePlugin): Promise<void> {
    retryOperation.value = () => confirmRemove(plugin);
    await store.remove(plugin);
    if (!store.operationError) {
      pendingAction.value = null;
      retryOperation.value = null;
    }
  }

  async function confirmCustomInstall(): Promise<void> {
    retryOperation.value = () => confirmCustomInstall();
    await store.installCustom();
    if (!store.operationError) retryOperation.value = null;
  }

  function dismissOperationError(): void {
    store.clearOperationError();
    pendingAction.value = null;
    retryOperation.value = null;
  }

  function retryFailedOperation(): void {
    const retry = retryOperation.value;
    if (!retry) return;
    store.clearOperationError();
    void retry();
  }

  function clearFilters(): void {
    store.search = "";
    store.category = "all";
  }

  function showCatalog(): void {
    compactPane.value = "catalog";
  }

  return {
    store,
    customExpanded,
    pendingAction,
    compactPane,
    isCompact,
    categories,
    filteredPlugins,
    installedCount,
    selectedPlugin,
    sourceStatus,
    retryOperation,
    selectPlugin,
    startCustomPreview,
    confirmInstall,
    confirmRemove,
    confirmCustomInstall,
    dismissOperationError,
    retryFailedOperation,
    clearFilters,
    showCatalog,
  };
}
