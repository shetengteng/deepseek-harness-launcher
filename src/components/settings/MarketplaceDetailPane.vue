<script setup lang="ts">
import MarketplaceCustomPreview from "@/components/settings/MarketplaceCustomPreview.vue";
import MarketplaceDetailFooter from "@/components/settings/MarketplaceDetailFooter.vue";
import MarketplacePluginDetail from "@/components/settings/MarketplacePluginDetail.vue";
import type { MarketplacePendingAction } from "@/lib/marketplace";
import type {
  MarketplaceCustomInstallPreview,
  MarketplacePlugin,
} from "@/lib/tauri";

interface Props {
  plugin: MarketplacePlugin | null;
  profile: string;
  customPreview: MarketplaceCustomInstallPreview | null;
  showCatalogBack: boolean;
}

defineProps<Props>();
const emit = defineEmits<{
  cancelCustom: [];
  confirmCustom: [];
  confirmInstall: [plugin: MarketplacePlugin];
  confirmRemove: [plugin: MarketplacePlugin];
  showCatalog: [];
}>();

const pendingAction = defineModel<MarketplacePendingAction>("pendingAction", {
  required: true,
});
</script>

<template>
  <article class="marketplace-detail-pane" aria-live="polite">
    <section class="marketplace-detail-body">
      <MarketplaceCustomPreview v-if="customPreview" :preview="customPreview" />
      <MarketplacePluginDetail
        v-else-if="plugin"
        :plugin="plugin"
        :profile="profile"
        :pending-action="pendingAction"
      />
      <div v-else class="marketplace-detail-empty">
        选择一个插件以查看详情。
      </div>
    </section>

    <MarketplaceDetailFooter
      :plugin="plugin"
      :profile="profile"
      :custom-preview="customPreview"
      :show-catalog-back="showCatalogBack"
      v-model:pending-action="pendingAction"
      @show-catalog="emit('showCatalog')"
      @cancel-custom="emit('cancelCustom')"
      @confirm-custom="emit('confirmCustom')"
      @confirm-install="emit('confirmInstall', $event)"
      @confirm-remove="emit('confirmRemove', $event)"
    />
  </article>
</template>

<style scoped>
.marketplace-detail-pane {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  background: hsl(var(--background));
}
.marketplace-detail-body {
  flex: 1;
  overflow: auto;
  padding: 22px 24px;
}
.marketplace-detail-empty {
  display: grid;
  min-height: 240px;
  place-items: center;
  color: hsl(var(--muted-foreground));
  font-size: 12px;
}
</style>
