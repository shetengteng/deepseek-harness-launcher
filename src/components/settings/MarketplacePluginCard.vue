<script setup lang="ts">
import { computed } from "vue";
import { ChevronRight, Code2, Package, Star } from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import {
  marketplaceDate,
  marketplaceInstallSpec,
  marketplaceRepositoryId,
  marketplaceSourceKind,
  marketplaceStatusLabel,
} from "@/lib/marketplace";
import type { MarketplacePlugin } from "@/lib/tauri";

interface Props {
  plugin: MarketplacePlugin;
  selected: boolean;
}

const props = defineProps<Props>();
const emit = defineEmits<{ select: [plugin: MarketplacePlugin] }>();

const initials = computed(() =>
  props.plugin.name
    .split(/[-_\s]+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase())
    .join(""),
);
const sourceKind = computed(() => marketplaceSourceKind(props.plugin));
const isInstalled = computed(
  () =>
    props.plugin.status === "installed" ||
    props.plugin.status === "update_available",
);
const stars = computed(() => props.plugin.popularity.github_stars);
</script>

<template>
  <button
    type="button"
    class="marketplace-plugin-card"
    :class="selected && 'is-selected'"
    :aria-selected="selected"
    role="option"
    @click="emit('select', plugin)"
  >
    <span class="marketplace-card-icon" aria-hidden="true">
      {{ initials || "D" }}
    </span>
    <span class="marketplace-card-main">
      <span class="marketplace-card-heading">
        <span class="marketplace-card-name">{{ plugin.name }}</span>
        <Badge
          v-if="isInstalled"
          variant="secondary"
          class="marketplace-card-status"
        >
          {{ marketplaceStatusLabel(plugin.status) }}
        </Badge>
        <Badge v-else variant="outline" class="marketplace-card-status">
          可安装
        </Badge>
      </span>
      <span class="marketplace-card-id">{{
        marketplaceRepositoryId(plugin)
      }}</span>
      <span class="marketplace-card-description">
        {{ plugin.description || "暂无描述" }}
      </span>
      <span class="marketplace-card-meta">
        <span v-if="plugin.category" class="marketplace-card-category">
          {{ plugin.category }}
        </span>
        <span class="marketplace-card-source">
          <Package
            v-if="sourceKind === 'npm'"
            class="h-3 w-3"
            aria-hidden="true"
          />
          <Code2 v-else class="h-3 w-3" aria-hidden="true" />
          # {{ sourceKind === "npm" ? "npm" : "源码" }}
        </span>
        <span class="marketplace-card-stars">
          <Star v-if="stars !== null" class="h-3 w-3" aria-hidden="true" />
          {{
            stars === null ? "Stars 未提供" : `${stars.toLocaleString()} Stars`
          }}
        </span>
        <span v-if="plugin.source_updated_at" class="marketplace-card-date">
          加入于 {{ marketplaceDate(plugin.source_updated_at) }}
        </span>
      </span>
    </span>
    <span
      class="marketplace-card-action"
      :class="isInstalled && 'is-installed'"
    >
      {{ isInstalled ? "已安装" : plugin.install_spec ? "查看" : "不可安装" }}
      <ChevronRight class="h-4 w-4" aria-hidden="true" />
    </span>
    <span v-if="plugin.local_only" class="sr-only">
      来源：{{ marketplaceInstallSpec(plugin) }}
    </span>
  </button>
</template>

<style scoped>
.marketplace-plugin-card {
  display: grid;
  width: 100%;
  grid-template-columns: 42px minmax(0, 1fr) auto;
  gap: 12px;
  min-height: 132px;
  padding: 15px;
  border: 1px solid hsl(var(--border));
  border-radius: 10px;
  background: hsl(var(--card));
  color: inherit;
  cursor: pointer;
  text-align: left;
  transition:
    border-color 150ms ease,
    background-color 150ms ease,
    transform 150ms ease;
}
.marketplace-plugin-card + .marketplace-plugin-card {
  margin-top: 8px;
}
.marketplace-plugin-card:hover {
  border-color: hsl(var(--input));
  background: hsl(var(--secondary) / 0.45);
}
.marketplace-plugin-card:active {
  transform: translateY(1px);
}
.marketplace-plugin-card.is-selected {
  border-color: hsl(var(--ring));
  background: hsl(var(--accent));
  box-shadow: 0 0 0 1px hsl(var(--ring) / 0.18);
}
.marketplace-card-icon {
  display: grid;
  width: 42px;
  height: 42px;
  place-items: center;
  border: 1px solid hsl(var(--border));
  border-radius: 12px;
  background: hsl(var(--secondary));
  color: hsl(var(--foreground));
  font-size: 13px;
  font-weight: 700;
  letter-spacing: 0.03em;
}
.marketplace-card-main {
  display: block;
  min-width: 0;
}
.marketplace-card-heading,
.marketplace-card-meta {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 7px;
}
.marketplace-card-heading {
  gap: 8px;
}
.marketplace-card-name {
  min-width: 0;
  overflow: hidden;
  color: hsl(var(--foreground));
  font-size: 14px;
  font-weight: 650;
  letter-spacing: -0.01em;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.marketplace-card-status {
  flex: 0 0 auto;
  padding: 2px 7px;
  font-size: 10px;
}
.marketplace-card-id {
  display: block;
  margin-top: 3px;
  overflow: hidden;
  color: hsl(var(--muted-foreground));
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.marketplace-card-description {
  display: -webkit-box;
  margin-top: 9px;
  overflow: hidden;
  color: hsl(var(--muted-foreground));
  font-size: 12px;
  line-height: 1.5;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
.marketplace-card-meta {
  flex-wrap: wrap;
  margin-top: 11px;
  color: hsl(var(--muted-foreground));
  font-size: 10px;
}
.marketplace-card-category,
.marketplace-card-source {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 7px;
  border-radius: 999px;
  background: hsl(var(--secondary));
}
.marketplace-card-source {
  color: hsl(var(--foreground));
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}
.marketplace-card-stars {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  white-space: nowrap;
}
.marketplace-card-stars :deep(svg) {
  color: hsl(var(--muted-foreground));
}
.marketplace-card-date {
  white-space: nowrap;
}
.marketplace-card-action {
  display: inline-flex;
  align-items: center;
  align-self: center;
  gap: 2px;
  color: hsl(var(--foreground));
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
}
.marketplace-card-action.is-installed {
  color: hsl(var(--muted-foreground));
}
@media (max-width: 620px) {
  .marketplace-plugin-card {
    grid-template-columns: 38px minmax(0, 1fr);
    padding: 13px;
  }
  .marketplace-card-icon {
    width: 38px;
    height: 38px;
  }
  .marketplace-card-action {
    display: none;
  }
}
</style>
