<script setup lang="ts">
import { computed, useTemplateRef } from "vue";
import { Button } from "@/components/ui/button";
import MarketplaceCatalogHeader from "@/components/settings/MarketplaceCatalogHeader.vue";
import MarketplaceCatalogToolbar from "@/components/settings/MarketplaceCatalogToolbar.vue";
import MarketplaceCustomInstall from "@/components/settings/MarketplaceCustomInstall.vue";
import MarketplacePluginCard from "@/components/settings/MarketplacePluginCard.vue";
import type { MarketplacePlugin, MarketplaceSort } from "@/lib/tauri";

interface Props {
  plugins: MarketplacePlugin[];
  categories: string[];
  loading: boolean;
  refreshing: boolean;
  customError: string | null;
  profiles: string[];
  sourceLabel: string;
  sourceUrl: string | null;
  sourceStale: boolean;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  select: [plugin: MarketplacePlugin];
  refresh: [];
  clearFilters: [];
  submitCustom: [];
}>();

const search = defineModel<string>("search", { required: true });
const category = defineModel<string>("category", { required: true });
const sort = defineModel<MarketplaceSort>("sort", { required: true });
const profile = defineModel<string>("profile", { required: true });
const selectedId = defineModel<string | null>("selectedId", { required: true });
const customExpanded = defineModel<boolean>("customExpanded", {
  required: true,
});
const customCommand = defineModel<string>("customCommand", { required: true });

const header =
  useTemplateRef<InstanceType<typeof MarketplaceCatalogHeader>>("header");
const hasActiveFilters = computed(
  () => Boolean(search.value) || category.value !== "all",
);

function focusSearch(): void {
  header.value?.focusSearch();
}

function selectWithKeyboard(event: KeyboardEvent): void {
  const current = props.plugins.findIndex(
    (plugin) => plugin.id === selectedId.value,
  );
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    const delta = event.key === "ArrowDown" ? 1 : -1;
    const next =
      props.plugins[current + delta] ??
      (delta > 0 ? props.plugins[0] : props.plugins[props.plugins.length - 1]);
    selectedId.value = next?.id ?? null;
  }
}

defineExpose({ focusSearch });
</script>

<template>
  <section class="marketplace-catalog-pane" aria-label="插件目录">
    <MarketplaceCatalogHeader
      ref="header"
      :categories="categories"
      :refreshing="refreshing"
      :source-label="sourceLabel"
      :source-url="sourceUrl"
      :source-stale="sourceStale"
      v-model:search="search"
      v-model:category="category"
      @refresh="emit('refresh')"
    >
      <MarketplaceCustomInstall
        :custom-error="customError"
        v-model:custom-expanded="customExpanded"
        v-model:custom-command="customCommand"
        @submit-custom="emit('submitCustom')"
      />
    </MarketplaceCatalogHeader>

    <MarketplaceCatalogToolbar
      :plugin-count="plugins.length"
      :source-stale="sourceStale"
      :has-active-filters="hasActiveFilters"
      :profiles="profiles"
      v-model:sort="sort"
      v-model:profile="profile"
      @clear-filters="emit('clearFilters')"
    />

    <div
      class="marketplace-result-list"
      tabindex="0"
      role="listbox"
      aria-label="插件结果"
      @keydown="selectWithKeyboard"
    >
      <div
        v-if="loading"
        class="marketplace-card-skeletons"
        aria-label="正在加载目录"
      >
        <div v-for="item in 5" :key="item" class="marketplace-card-skeleton">
          <div class="h-10 w-10 animate-pulse rounded-xl bg-muted" />
          <div class="min-w-0 flex-1 space-y-2">
            <div class="h-3 w-2/5 animate-pulse rounded bg-muted" />
            <div class="h-3 w-4/5 animate-pulse rounded bg-muted" />
            <div class="h-3 w-3/5 animate-pulse rounded bg-muted" />
          </div>
        </div>
      </div>
      <div v-else-if="!plugins.length" class="marketplace-empty-list">
        <p>没有匹配的插件。</p>
        <Button
          v-if="hasActiveFilters"
          variant="outline"
          size="sm"
          class="mt-3"
          @click="emit('clearFilters')"
          >清除筛选</Button
        >
      </div>
      <MarketplacePluginCard
        v-for="plugin in plugins"
        v-else
        :key="plugin.id"
        :plugin="plugin"
        :selected="selectedId === plugin.id"
        @select="emit('select', plugin)"
      />
    </div>
  </section>
</template>

<style scoped>
.marketplace-catalog-pane {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  border-right: 1px solid hsl(var(--border));
  background: hsl(var(--background));
}
.marketplace-result-list {
  flex: 1;
  overflow: auto;
  padding: 10px 12px 18px;
  outline: none;
}
.marketplace-card-skeletons {
  display: grid;
  gap: 8px;
  padding: 2px;
}
.marketplace-card-skeleton {
  display: flex;
  gap: 12px;
  min-height: 128px;
  padding: 16px;
  border: 1px solid hsl(var(--border));
  border-radius: 10px;
}
.marketplace-empty-list {
  padding: 54px 20px;
  color: hsl(var(--muted-foreground));
  font-size: 12px;
  text-align: center;
}
@media (max-width: 620px) {
  .marketplace-result-list {
    padding-inline: 8px;
  }
}
</style>
