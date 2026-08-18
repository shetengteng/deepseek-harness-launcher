<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import {
  ChevronDown,
  ExternalLink,
  RefreshCw,
  Search,
  Star,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useMarketplaceStore } from "@/stores/marketplace";
import type { MarketplacePlugin } from "@/lib/tauri";

const store = useMarketplaceStore();
const searchInput = ref<InstanceType<typeof Input> | null>(null);
const customExpanded = ref(false);
const pendingAction = ref<"install" | "remove" | null>(null);

const categories = computed(() => [
  ...new Set(
    (store.snapshot?.plugins ?? [])
      .map((plugin) => plugin.category)
      .filter((category): category is string => category !== null),
  ),
]);

const filteredPlugins = computed(() => {
  const query = store.search.trim().toLocaleLowerCase();
  const rankLimit =
    store.rankLimit === "top100"
      ? 100
      : store.rankLimit === "top500"
        ? 500
        : null;
  const starMinimum = store.starMinimum === "all" ? 0 : Number(store.starMinimum);
  const plugins = (store.snapshot?.plugins ?? []).filter((plugin) => {
    const matchesTab =
      store.tab === "market"
        ? !plugin.local_only
        : plugin.status === "installed" || plugin.status === "update_available";
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
      (store.category === "all" || plugin.category === store.category) &&
      (!rankLimit ||
        (plugin.popularity.marketplace_rank !== null &&
          plugin.popularity.marketplace_rank <= rankLimit)) &&
      (!starMinimum ||
        (plugin.popularity.github_stars !== null &&
          plugin.popularity.github_stars >= starMinimum))
    );
  });
  return [...plugins].sort((left, right) => {
    if (store.sort === "updated") {
      return (right.source_updated_at ?? "").localeCompare(
        left.source_updated_at ?? "",
      );
    }
    if (store.sort === "relevance") return left.name.localeCompare(right.name);
    return (
      (left.popularity.marketplace_rank ?? Number.MAX_SAFE_INTEGER) -
      (right.popularity.marketplace_rank ?? Number.MAX_SAFE_INTEGER)
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

const selectedPlugin = computed(() => {
  const selected = store.selectedPlugin;
  return selected && filteredPlugins.value.some((plugin) => plugin.id === selected.id)
    ? selected
    : filteredPlugins.value[0] ?? null;
});

watch(
  filteredPlugins,
  (plugins) => {
    if (!plugins.some((plugin) => plugin.id === store.selectedId)) {
      store.selectedId = plugins[0]?.id ?? null;
      pendingAction.value = null;
    }
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
    searchInput.value?.$el.focus();
  }
}

function selectWithKeyboard(event: KeyboardEvent): void {
  const current = filteredPlugins.value.findIndex(
    (plugin) => plugin.id === store.selectedId,
  );
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    const delta = event.key === "ArrowDown" ? 1 : -1;
    const next = filteredPlugins.value[current + delta] ??
      (delta > 0
        ? filteredPlugins.value[0]
        : filteredPlugins.value[filteredPlugins.value.length - 1]);
    store.selectedId = next?.id ?? null;
  }
  if (event.key === "Enter" && store.selectedId) {
    event.preventDefault();
    pendingAction.value = null;
  }
}

function selectPlugin(plugin: MarketplacePlugin): void {
  store.selectedId = plugin.id;
  pendingAction.value = null;
  store.operationError = null;
}

function installSpec(plugin: MarketplacePlugin): string {
  if (!plugin.install_spec) return plugin.installed_source ?? "目录未提供";
  const { owner, repository, subdirectory } = plugin.install_spec;
  return `github:${owner}/${repository}${subdirectory ? `#path:${subdirectory}` : ""}`;
}

function displayDate(value: string | null): string {
  if (!value) return "目录未提供";
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleDateString();
}

function startCustomPreview(): void {
  void (async () => {
    await store.previewCustomInstall();
    if (store.customPreview) pendingAction.value = null;
  })();
}

async function confirmInstall(plugin: MarketplacePlugin): Promise<void> {
  await store.install(plugin);
  if (!store.operationError) pendingAction.value = null;
}

async function confirmRemove(plugin: MarketplacePlugin): Promise<void> {
  await store.remove(plugin);
  if (!store.operationError) pendingAction.value = null;
}

async function confirmCustomInstall(): Promise<void> {
  await store.installCustom();
}

function clearFilters(): void {
  store.search = "";
  store.category = "all";
  store.rankLimit = "all";
  store.starMinimum = "all";
}

function switchTab(tab: "market" | "installed"): void {
  store.tab = tab;
  pendingAction.value = null;
  if (tab === "installed") store.customPreview = null;
}
</script>

<template>
  <section class="flex min-h-0 flex-1 flex-col" aria-labelledby="marketplace-title">
    <header class="shrink-0 border-b px-6 py-5">
      <div class="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h1 id="marketplace-title" class="text-lg font-semibold">插件市场</h1>
          <p class="mt-1 text-sm text-muted-foreground">
            从受控目录安装到当前 dsh profile，或管理本地已安装插件。
          </p>
        </div>
        <div class="flex items-center gap-2 text-xs text-muted-foreground" aria-live="polite">
          <span v-if="store.snapshot?.source.stale" class="text-amber-600 dark:text-amber-400">
            正在显示上次同步的目录，结果可能不是最新。
          </span>
          <span v-else-if="store.snapshot?.source.fetched_at">
            {{ store.snapshot.source.label }} · 已同步于 {{ displayDate(store.snapshot.source.fetched_at) }}
          </span>
          <Button variant="outline" size="xs" :disabled="store.refreshing" @click="store.refresh">
            <RefreshCw :class="['h-3.5 w-3.5', store.refreshing && 'animate-spin']" />
            {{ store.refreshing ? "刷新中" : "刷新目录" }}
          </Button>
        </div>
      </div>
      <div class="mt-5 flex items-center gap-1" role="tablist" aria-label="插件范围">
        <Button
          size="sm"
          :variant="store.tab === 'market' ? 'secondary' : 'ghost'"
          role="tab"
          :aria-selected="store.tab === 'market'"
          @click="switchTab('market')"
        >市场</Button>
        <Button
          size="sm"
          :variant="store.tab === 'installed' ? 'secondary' : 'ghost'"
          role="tab"
          :aria-selected="store.tab === 'installed'"
          @click="switchTab('installed')"
        >已安装 <span class="tabular-nums">{{ installedCount }}</span></Button>
      </div>
    </header>

    <p v-if="store.error" class="shrink-0 border-b border-destructive/40 bg-destructive/10 px-6 py-3 text-sm text-destructive" role="alert">
      {{ store.error }}
      <Button variant="link" size="xs" class="ml-1 h-auto px-0 text-destructive" @click="store.load">重试</Button>
    </p>

    <div class="grid min-h-0 flex-1 grid-cols-[minmax(320px,0.9fr)_minmax(360px,1.1fr)] overflow-hidden max-[900px]:block">
      <div class="flex min-h-0 flex-col border-r max-[900px]:border-r-0">
        <div class="shrink-0 border-b p-4">
          <div class="relative">
            <Search class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input ref="searchInput" v-model="store.search" class="pl-9" type="search" placeholder="搜索名称、仓库或能力" aria-label="搜索插件" />
          </div>
          <div class="mt-3 grid grid-cols-4 gap-2 max-[900px]:grid-cols-2" aria-label="插件筛选条件">
            <Select v-model="store.category"><SelectTrigger><SelectValue placeholder="全部分类" /></SelectTrigger><SelectContent><SelectItem value="all">全部分类</SelectItem><SelectItem v-for="item in categories" :key="item" :value="item">{{ item }}</SelectItem></SelectContent></Select>
            <Select v-model="store.sort"><SelectTrigger><SelectValue /></SelectTrigger><SelectContent><SelectItem value="popularity">市场排名</SelectItem><SelectItem value="relevance">相关性</SelectItem><SelectItem value="updated">最近更新</SelectItem></SelectContent></Select>
            <Select v-model="store.rankLimit"><SelectTrigger><SelectValue /></SelectTrigger><SelectContent><SelectItem value="all">全部排名</SelectItem><SelectItem value="top100">Top 100</SelectItem><SelectItem value="top500">Top 500</SelectItem></SelectContent></Select>
            <Select v-model="store.starMinimum"><SelectTrigger><SelectValue /></SelectTrigger><SelectContent><SelectItem value="all">不限 Stars</SelectItem><SelectItem value="100">100+ Stars</SelectItem><SelectItem value="250">250+ Stars</SelectItem></SelectContent></Select>
          </div>

          <div v-if="store.tab === 'market'" class="mt-3">
            <Button
              variant="ghost"
              size="xs"
              class="gap-1 px-1 text-muted-foreground"
              :aria-expanded="customExpanded"
              aria-controls="custom-install-content"
              @click="customExpanded = !customExpanded"
            ><span class="font-mono text-sm">+</span> 自定义安装 <ChevronDown :class="['h-3.5 w-3.5 transition-transform', customExpanded && 'rotate-180']" /></Button>
            <form v-if="customExpanded" id="custom-install-content" class="mt-2 flex gap-2 max-[620px]:flex-col" @submit.prevent="startCustomPreview">
              <Input v-model="store.customCommand" class="font-mono text-xs" placeholder="dsh plugin --profile web add <source>" aria-label="自定义插件安装命令" :aria-invalid="Boolean(store.customError)" :aria-describedby="store.customError ? 'custom-command-error' : undefined" />
              <Button type="submit" class="shrink-0">继续</Button>
            </form>
            <p v-if="store.customError" id="custom-command-error" class="mt-2 text-xs text-destructive" role="alert">{{ store.customError }}</p>
          </div>

          <div class="mt-3 flex items-center justify-between text-xs text-muted-foreground">
            <span>{{ filteredPlugins.length }} 个结果</span>
            <Select v-model="store.profile"><SelectTrigger class="h-7 w-28 text-xs"><SelectValue /></SelectTrigger><SelectContent><SelectItem v-for="item in store.profiles" :key="item" :value="item">{{ item }}</SelectItem></SelectContent></Select>
          </div>
        </div>

        <div class="min-h-0 flex-1 overflow-y-auto p-2" tabindex="0" aria-label="插件结果" @keydown="selectWithKeyboard">
          <div v-if="store.loading" class="space-y-2 p-2" aria-label="正在加载目录">
            <div v-for="item in 6" :key="item" class="h-24 animate-pulse rounded-md bg-muted" />
          </div>
          <div v-else-if="!filteredPlugins.length" class="px-6 py-14 text-center text-sm text-muted-foreground">
            <p>{{ store.tab === 'installed' ? '当前 profile 没有已安装插件。' : '没有匹配的插件。可清除筛选或调整关键词。' }}</p>
            <Button v-if="store.tab === 'market'" class="mt-3" variant="outline" size="sm" @click="clearFilters">清除筛选</Button>
          </div>
          <button
            v-for="plugin in filteredPlugins"
            :key="plugin.id"
            type="button"
            class="mb-1 flex w-full flex-col rounded-md border p-3 text-left transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            :class="store.selectedId === plugin.id ? 'border-ring bg-accent/60' : 'border-transparent'"
            :aria-current="store.selectedId === plugin.id ? 'true' : undefined"
            @click="selectPlugin(plugin)"
          >
            <span class="flex items-center gap-2"><Badge variant="outline" class="shrink-0 font-mono text-[10px]">{{ plugin.local_only ? '本地' : plugin.popularity.marketplace_rank ? `#${plugin.popularity.marketplace_rank}` : '未上榜' }}</Badge><span class="truncate font-medium">{{ plugin.name }}</span><Badge v-if="plugin.status === 'installed' || plugin.status === 'update_available'" variant="secondary">{{ plugin.status === 'update_available' ? '有可用更新' : '已安装' }}</Badge><Badge v-else variant="outline">可安装</Badge></span>
            <span class="mt-1 truncate font-mono text-xs text-muted-foreground">{{ plugin.local_only ? plugin.installed_source : plugin.id }}</span>
            <span class="mt-1 line-clamp-2 text-xs leading-5 text-muted-foreground">{{ plugin.description }}</span>
            <span class="mt-2 flex items-center gap-2 text-[11px] text-muted-foreground"><Badge v-if="plugin.category" variant="outline" class="font-normal">{{ plugin.category }}</Badge><span v-if="plugin.popularity.github_stars !== null" class="inline-flex items-center gap-1"><Star class="h-3 w-3" aria-hidden="true" />{{ plugin.popularity.github_stars.toLocaleString() }} Stars</span><span v-else-if="!plugin.local_only">目录未提供 Stars</span><span v-if="plugin.source_updated_at">更新于 {{ displayDate(plugin.source_updated_at) }}</span></span>
          </button>
        </div>
      </div>

      <article class="min-h-0 overflow-y-auto p-6 max-[900px]:border-t" aria-live="polite">
        <section v-if="store.customPreview" aria-labelledby="custom-preview-title">
          <p class="text-xs font-medium text-muted-foreground">自定义安装</p>
          <h2 id="custom-preview-title" class="mt-1 text-lg font-semibold">自定义来源，尚未执行</h2>
          <p class="mt-3 text-sm leading-6 text-muted-foreground">此来源未经过市场目录校验。Launcher 只会使用后端已解析的字段重新构造 dsh 参数。</p>
          <dl class="mt-5 divide-y rounded-md border text-sm"><div class="grid grid-cols-[110px_1fr] gap-3 p-3"><dt class="text-muted-foreground">目标 profile</dt><dd class="font-mono">{{ store.customPreview.profile }}</dd></div><div class="grid grid-cols-[110px_1fr] gap-3 p-3"><dt class="text-muted-foreground">来源</dt><dd class="break-all font-mono text-xs">{{ store.customPreview.source }}</dd></div><div class="grid grid-cols-[110px_1fr] gap-3 p-3"><dt class="text-muted-foreground">执行者</dt><dd>Launcher 托管的 dsh {{ store.customPreview.dsh_version }}</dd></div></dl>
          <div class="mt-5 flex justify-end gap-2"><Button variant="outline" @click="store.returnToCustomCommand">返回编辑</Button><Button @click="confirmCustomInstall">确认安装</Button></div>
        </section>

        <section v-else-if="selectedPlugin" :aria-labelledby="`plugin-detail-${selectedPlugin.id}`">
          <div class="flex items-start justify-between gap-3"><div><p class="text-xs text-muted-foreground">{{ selectedPlugin.local_only ? '本地插件' : '目录插件' }}</p><h2 :id="`plugin-detail-${selectedPlugin.id}`" class="mt-1 text-lg font-semibold">{{ selectedPlugin.name }}</h2></div><Badge :variant="selectedPlugin.status === 'installed' ? 'secondary' : 'outline'">{{ selectedPlugin.status === 'installed' ? '已安装' : selectedPlugin.status === 'unknown' ? '状态待确认' : '可安装' }}</Badge></div>
          <p class="mt-4 text-sm leading-6 text-muted-foreground">{{ selectedPlugin.description }}</p>
          <a v-if="selectedPlugin.repository_url" :href="selectedPlugin.repository_url" target="_blank" rel="noreferrer" class="mt-4 inline-flex items-center gap-1 text-sm underline underline-offset-4">{{ selectedPlugin.repository_url }}<ExternalLink class="h-3.5 w-3.5" /></a>
          <dl class="mt-5 divide-y rounded-md border text-sm"><div class="grid grid-cols-[110px_1fr] gap-3 p-3"><dt class="text-muted-foreground">安装来源</dt><dd class="break-all font-mono text-xs">{{ installSpec(selectedPlugin) }}</dd></div><div class="grid grid-cols-[110px_1fr] gap-3 p-3"><dt class="text-muted-foreground">目标 profile</dt><dd class="font-mono">{{ store.profile }}</dd></div><div class="grid grid-cols-[110px_1fr] gap-3 p-3"><dt class="text-muted-foreground">市场排名</dt><dd>{{ selectedPlugin.popularity.marketplace_rank ? `#${selectedPlugin.popularity.marketplace_rank}` : '目录未提供' }}</dd></div><div class="grid grid-cols-[110px_1fr] gap-3 p-3"><dt class="text-muted-foreground">GitHub Stars</dt><dd>{{ selectedPlugin.popularity.github_stars?.toLocaleString() ?? '目录未提供' }}</dd></div><div class="grid grid-cols-[110px_1fr] gap-3 p-3"><dt class="text-muted-foreground">最后更新</dt><dd>{{ displayDate(selectedPlugin.source_updated_at) }}</dd></div><div class="grid grid-cols-[110px_1fr] gap-3 p-3"><dt class="text-muted-foreground">静态校验</dt><dd>{{ displayDate(selectedPlugin.validated_at) }}</dd></div></dl>
          <div v-if="selectedPlugin.tags.length" class="mt-4 flex flex-wrap gap-1"><Badge v-for="tag in selectedPlugin.tags" :key="tag" variant="outline" class="font-normal">{{ tag }}</Badge></div>

          <div v-if="pendingAction === 'install'" class="mt-5 rounded-md border bg-muted/40 p-4"><p class="font-medium">确认安装</p><p class="mt-2 text-sm text-muted-foreground">将安装到 profile：{{ store.profile }}</p><code class="mt-2 block break-all rounded bg-background p-2 text-xs">dsh plugin --profile {{ store.profile }} add {{ installSpec(selectedPlugin) }}</code><div class="mt-4 flex justify-end gap-2"><Button variant="outline" @click="pendingAction = null">取消</Button><Button @click="confirmInstall(selectedPlugin)">确认安装</Button></div></div>
          <div v-else-if="pendingAction === 'remove'" class="mt-5 rounded-md border border-destructive/40 bg-destructive/5 p-4"><p class="font-medium">确认卸载</p><p class="mt-2 text-sm text-muted-foreground">将从 profile {{ store.profile }} 移除本地安装：<code class="break-all">{{ selectedPlugin.installed_source }}</code></p><div class="mt-4 flex justify-end gap-2"><Button variant="outline" @click="pendingAction = null">取消</Button><Button variant="destructive" @click="confirmRemove(selectedPlugin)">确认卸载</Button></div></div>
          <div v-else class="mt-5 flex justify-end"><Button v-if="selectedPlugin.status === 'installed' || selectedPlugin.status === 'update_available'" variant="destructive" @click="pendingAction = 'remove'">卸载</Button><Button v-else :disabled="selectedPlugin.status === 'unknown'" @click="pendingAction = 'install'">安装</Button></div>
          <p v-if="store.operationError" class="mt-3 text-sm text-destructive" role="alert">{{ store.operationError }}<span v-if="store.operation?.log_path" class="mt-1 block break-all font-mono text-xs text-muted-foreground">日志：{{ store.operation.log_path }}</span></p>
          <p class="mt-7 border-t pt-4 text-xs leading-5 text-muted-foreground">目录校验插件结构，不代表代码安全。安装前请确认仓库来源；插件可在 dsh 中执行其声明的能力。</p>
        </section>
        <div v-else class="flex min-h-64 items-center justify-center text-sm text-muted-foreground">选择一个插件以查看详情。</div>
      </article>
    </div>
  </section>
</template>
