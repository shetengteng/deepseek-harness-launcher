<script setup lang="ts">
import { ref } from "vue";
import { ExternalLink, RefreshCw, Search } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface Props {
  categories: string[];
  refreshing: boolean;
  sourceLabel: string;
  sourceUrl: string | null;
  sourceStale: boolean;
}

defineProps<Props>();
const emit = defineEmits<{
  refresh: [];
}>();

const search = defineModel<string>("search", { required: true });
const category = defineModel<string>("category", { required: true });
const searchInput = ref<InstanceType<typeof Input> | null>(null);

function focusSearch(): void {
  searchInput.value?.$el.focus();
}

defineExpose({ focusSearch });
</script>

<template>
  <header class="marketplace-catalog-header">
    <div class="marketplace-catalog-heading">
      <div class="marketplace-directory-source" aria-label="目录来源">
        <span :class="sourceStale && 'is-stale'">
          {{ sourceStale ? "目录可能已过期" : "目录已同步" }}
        </span>
        <a
          v-if="sourceUrl"
          :href="sourceUrl"
          target="_blank"
          rel="noreferrer"
          class="marketplace-directory-link"
        >
          {{ sourceLabel }}
          <ExternalLink class="h-3 w-3" aria-hidden="true" />
        </a>
      </div>
      <Button
        variant="ghost"
        size="xs"
        class="marketplace-refresh-button"
        :disabled="refreshing"
        @click="emit('refresh')"
      >
        <RefreshCw
          :class="['h-4 w-4', refreshing && 'animate-spin']"
          aria-hidden="true"
        />
        {{ refreshing ? "刷新中" : "刷新" }}
      </Button>
    </div>

    <div class="marketplace-search-wrap">
      <Search class="marketplace-search-icon" aria-hidden="true" />
      <Input
        ref="searchInput"
        v-model="search"
        class="marketplace-search-input h-10"
        type="search"
        placeholder="搜索名称、仓库或能力"
        aria-label="搜索插件"
      />
      <kbd class="marketplace-search-shortcut">⌘ K</kbd>
    </div>

    <div class="marketplace-category-bar" aria-label="插件分类">
      <button
        type="button"
        class="marketplace-category-chip"
        :class="category === 'all' && 'is-active'"
        @click="category = 'all'"
      >
        全部
      </button>
      <button
        v-for="item in categories"
        :key="item"
        type="button"
        class="marketplace-category-chip"
        :class="category === item && 'is-active'"
        @click="category = item"
      >
        {{ item }}
      </button>
    </div>
    <slot />
  </header>
</template>

<style scoped>
.marketplace-catalog-header {
  padding: 20px 22px 15px;
  border-bottom: 1px solid hsl(var(--border));
}
.marketplace-catalog-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 14px;
}
.marketplace-directory-source {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 9px;
  color: hsl(var(--muted-foreground));
  font-size: 10px;
}
.marketplace-directory-source .is-stale {
  color: hsl(var(--destructive));
}
.marketplace-directory-link {
  display: inline-flex;
  min-width: 0;
  align-items: center;
  gap: 3px;
  color: hsl(var(--muted-foreground));
  text-decoration: none;
}
.marketplace-directory-link:hover {
  color: hsl(var(--foreground));
  text-decoration: underline;
  text-underline-offset: 3px;
}
.marketplace-refresh-button {
  color: hsl(var(--muted-foreground));
}
.marketplace-refresh-button:hover {
  color: hsl(var(--foreground));
}
.marketplace-search-wrap {
  position: relative;
}
.marketplace-search-icon {
  pointer-events: none;
  position: absolute;
  top: 50%;
  left: 12px;
  z-index: 1;
  width: 15px;
  height: 15px;
  transform: translateY(-50%);
  color: hsl(var(--muted-foreground));
}
.marketplace-search-input {
  padding-left: 38px;
  padding-right: 60px;
  border-radius: 9px;
  background: hsl(var(--secondary) / 0.45);
}
.marketplace-search-shortcut {
  pointer-events: none;
  position: absolute;
  top: 50%;
  right: 9px;
  transform: translateY(-50%);
  color: hsl(var(--muted-foreground));
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 10px;
}
.marketplace-category-bar {
  display: flex;
  gap: 6px;
  margin-top: 14px;
  overflow-x: auto;
  padding-bottom: 2px;
  scrollbar-width: thin;
}
.marketplace-category-chip {
  flex: 0 0 auto;
  min-height: 28px;
  padding: 5px 10px;
  border: 1px solid transparent;
  border-radius: 999px;
  background: transparent;
  color: hsl(var(--muted-foreground));
  cursor: pointer;
  font-size: 11px;
  white-space: nowrap;
}
.marketplace-category-chip:hover {
  background: hsl(var(--secondary));
  color: hsl(var(--foreground));
}
.marketplace-category-chip.is-active {
  border-color: hsl(var(--border));
  background: hsl(var(--secondary));
  color: hsl(var(--foreground));
  font-weight: 600;
}
@media (max-width: 620px) {
  .marketplace-catalog-header {
    padding-inline: 14px;
  }
}
</style>
