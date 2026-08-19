<script setup lang="ts">
import MarketplaceCatalogPane from "@/components/settings/MarketplaceCatalogPane.vue";
import MarketplaceDetailPane from "@/components/settings/MarketplaceDetailPane.vue";
import MarketplaceOperationErrorDialog from "@/components/settings/MarketplaceOperationErrorDialog.vue";
import { Button } from "@/components/ui/button";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useMarketplaceWorkspace } from "@/composables/useMarketplaceWorkspace";

const {
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
} = useMarketplaceWorkspace();
</script>

<template>
  <section class="marketplace-workspace" aria-label="插件市场">
    <header class="marketplace-topbar">
      <Tabs
        v-model="store.tab"
        class="marketplace-tabs-root"
        activation-mode="manual"
      >
        <TabsList class="marketplace-tabs" aria-label="插件范围">
          <TabsTrigger class="marketplace-tab-trigger" value="discover"
            >发现</TabsTrigger
          >
          <TabsTrigger class="marketplace-tab-trigger" value="installed">
            已安装
            <span class="marketplace-tab-count">{{ installedCount }}</span>
          </TabsTrigger>
        </TabsList>
      </Tabs>
    </header>

    <p v-if="store.error" class="marketplace-error" role="alert">
      {{ store.error }}
      <Button
        variant="link"
        size="xs"
        class="ml-1 h-auto px-0 text-destructive"
        @click="store.load"
        >重试</Button
      >
    </p>

    <div class="marketplace-layout">
      <MarketplaceCatalogPane
        v-show="!isCompact || compactPane === 'catalog'"
        ref="catalogPane"
        :plugins="filteredPlugins"
        :categories="categories"
        :loading="store.loading"
        :refreshing="store.refreshing"
        :custom-error="store.customError"
        :profiles="store.profiles"
        :source-label="sourceStatus?.label ?? '目录来源'"
        :source-url="sourceStatus?.url ?? null"
        :source-stale="sourceStatus?.stale ?? false"
        v-model:search="store.search"
        v-model:category="store.category"
        v-model:sort="store.sort"
        v-model:profile="store.profile"
        v-model:selected-id="store.selectedId"
        v-model:custom-expanded="customExpanded"
        v-model:custom-command="store.customCommand"
        @select="selectPlugin"
        @refresh="store.refresh"
        @clear-filters="clearFilters"
        @submit-custom="startCustomPreview"
      />
      <MarketplaceDetailPane
        v-show="!isCompact || compactPane === 'detail'"
        :plugin="selectedPlugin"
        :profile="store.profile"
        :custom-preview="store.customPreview"
        :show-catalog-back="isCompact"
        v-model:pending-action="pendingAction"
        @show-catalog="showCatalog"
        @cancel-custom="store.returnToCustomCommand"
        @confirm-custom="confirmCustomInstall"
        @confirm-install="confirmInstall"
        @confirm-remove="confirmRemove"
      />
    </div>

    <MarketplaceOperationErrorDialog
      :error="store.operationError"
      :operation="store.operation"
      :can-retry="retryOperation !== null"
      @dismiss="dismissOperationError"
      @retry="retryFailedOperation"
    />
  </section>
</template>

<style scoped>
.marketplace-workspace {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
}
.marketplace-topbar {
  padding: 0 26px;
  border-bottom: 1px solid hsl(var(--border));
  background: hsl(var(--background));
}
.marketplace-tabs-root {
  margin: 0;
}
.marketplace-tabs {
  display: flex;
  align-items: stretch;
  gap: 4px;
  border-radius: 0;
  background: transparent;
  padding: 0;
}
.marketplace-tab-trigger {
  min-height: 38px;
  border-bottom: 2px solid transparent;
  border-radius: 0;
  padding: 7px 12px;
  color: hsl(var(--muted-foreground));
  font-size: 12px;
}
.marketplace-tab-trigger[data-state="active"] {
  border-bottom-color: hsl(var(--foreground));
  background: transparent;
  color: hsl(var(--foreground));
  font-weight: 650;
  box-shadow: none;
}
.marketplace-tab-count {
  margin-left: 4px;
  color: hsl(var(--muted-foreground));
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 10px;
}
.marketplace-error {
  margin: 0;
  padding: 10px 26px;
  border-bottom: 1px solid hsl(var(--destructive) / 0.4);
  background: hsl(var(--destructive) / 0.08);
  color: hsl(var(--destructive));
  font-size: 12px;
}
.marketplace-layout {
  display: grid;
  min-height: 0;
  flex: 1;
  grid-template-columns: minmax(400px, 0.98fr) minmax(420px, 1.02fr);
  overflow: hidden;
}
@media (max-width: 899px) {
  .marketplace-layout {
    grid-template-columns: minmax(0, 1fr);
  }
}
@media (max-width: 620px) {
  .marketplace-topbar {
    padding-inline: 14px;
  }
  .marketplace-error {
    padding-inline: 14px;
  }
}
</style>
