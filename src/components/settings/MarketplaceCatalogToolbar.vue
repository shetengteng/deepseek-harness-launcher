<script setup lang="ts">
import { SlidersHorizontal } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { MarketplaceSort } from "@/lib/tauri";

interface Props {
  pluginCount: number;
  sourceStale: boolean;
  hasActiveFilters: boolean;
  profiles: string[];
}

defineProps<Props>();
const emit = defineEmits<{
  clearFilters: [];
}>();

const sort = defineModel<MarketplaceSort>("sort", { required: true });
const profile = defineModel<string>("profile", { required: true });
</script>

<template>
  <div class="marketplace-catalog-toolbar">
    <div class="marketplace-catalog-count">
      <strong>{{ pluginCount }}</strong>
      <span>个结果</span>
      <span v-if="sourceStale" class="marketplace-stale-label">可能已过期</span>
    </div>
    <div class="marketplace-toolbar-actions">
      <Button
        v-if="hasActiveFilters"
        variant="ghost"
        size="xs"
        @click="emit('clearFilters')"
      >
        清除
      </Button>
      <Select v-model="sort">
        <SelectTrigger class="marketplace-sort-trigger h-8 text-xs">
          <SlidersHorizontal class="mr-1.5 h-3.5 w-3.5" aria-hidden="true" />
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="relevance">推荐</SelectItem>
          <SelectItem value="updated">最近加入</SelectItem>
          <SelectItem value="stars">GitHub Stars</SelectItem>
        </SelectContent>
      </Select>
      <Select v-model="profile">
        <SelectTrigger class="marketplace-profile-trigger h-8 text-xs">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="item in profiles" :key="item" :value="item">
            {{ item }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>
  </div>
</template>

<style scoped>
.marketplace-catalog-toolbar {
  display: flex;
  min-height: 45px;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 0 22px;
  border-bottom: 1px solid hsl(var(--border));
  color: hsl(var(--muted-foreground));
  font-size: 11px;
}
.marketplace-catalog-count,
.marketplace-toolbar-actions {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 6px;
}
.marketplace-catalog-count strong {
  color: hsl(var(--foreground));
  font-weight: 650;
}
.marketplace-stale-label {
  color: hsl(var(--destructive));
}
.marketplace-sort-trigger,
.marketplace-profile-trigger {
  justify-content: flex-start;
  border-color: transparent;
  background: transparent;
}
.marketplace-sort-trigger {
  min-width: 116px;
}
.marketplace-profile-trigger {
  min-width: 76px;
}
.marketplace-sort-trigger:hover,
.marketplace-profile-trigger:hover {
  border-color: hsl(var(--input));
  background: hsl(var(--secondary));
}
@media (max-width: 620px) {
  .marketplace-catalog-toolbar {
    padding-inline: 14px;
  }
}
</style>
