<script setup lang="ts">
import { computed } from "vue";
import { Button } from "@/components/ui/button";
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

const props = defineProps<Props>();
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

const showRemove = computed(
  () =>
    props.plugin?.status === "installed" ||
    props.plugin?.status === "update_available",
);
</script>

<template>
  <footer class="marketplace-detail-footer">
    <div class="marketplace-detail-footer-start">
      <Button
        v-if="showCatalogBack"
        variant="ghost"
        size="xs"
        @click="emit('showCatalog')"
      >
        返回列表
      </Button>
      <span class="marketplace-profile-line">
        目标 profile：<strong class="font-mono">{{ profile }}</strong>
      </span>
    </div>

    <div class="flex items-center justify-end gap-2">
      <template v-if="customPreview">
        <Button
          variant="outline"
          size="xs"
          class="rounded-full px-3"
          @click="emit('cancelCustom')"
          >返回编辑</Button
        >
        <Button
          size="xs"
          class="rounded-full px-3"
          @click="emit('confirmCustom')"
          >确认安装</Button
        >
      </template>
      <template v-else-if="plugin && pendingAction === 'install'">
        <Button
          variant="outline"
          size="xs"
          class="rounded-full px-3"
          @click="pendingAction = null"
          >取消</Button
        >
        <Button
          size="xs"
          class="rounded-full px-3"
          @click="emit('confirmInstall', plugin)"
          >确认安装</Button
        >
      </template>
      <template v-else-if="plugin && pendingAction === 'remove'">
        <Button
          variant="outline"
          size="xs"
          class="rounded-full px-3"
          @click="pendingAction = null"
          >取消</Button
        >
        <Button
          variant="destructive"
          size="xs"
          class="rounded-full px-3"
          @click="emit('confirmRemove', plugin)"
          >确认卸载</Button
        >
      </template>
      <template v-else-if="plugin">
        <Button
          v-if="showRemove"
          variant="destructive"
          size="xs"
          class="rounded-full px-3"
          @click="pendingAction = 'remove'"
          >卸载</Button
        >
        <Button
          v-else
          size="xs"
          class="rounded-full px-3"
          :disabled="plugin.status === 'unknown'"
          @click="pendingAction = 'install'"
          >安装</Button
        >
      </template>
    </div>
  </footer>
</template>

<style scoped>
.marketplace-detail-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-height: 58px;
  padding: 12px 24px;
  border-top: 1px solid hsl(var(--border));
}
.marketplace-detail-footer-start {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.marketplace-profile-line {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: hsl(var(--muted-foreground));
  font-size: 11px;
}
@media (max-width: 620px) {
  .marketplace-detail-footer {
    flex-wrap: wrap;
  }
}
</style>
