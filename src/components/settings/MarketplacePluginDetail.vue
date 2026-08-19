<script setup lang="ts">
import { ExternalLink } from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import {
  marketplaceDate,
  marketplaceInstallSpec,
  marketplaceStatusLabel,
  type MarketplacePendingAction,
} from "@/lib/marketplace";
import type { MarketplacePlugin } from "@/lib/tauri";

interface Props {
  plugin: MarketplacePlugin;
  profile: string;
  pendingAction: MarketplacePendingAction;
}

defineProps<Props>();
</script>

<template>
  <div>
    <div class="marketplace-detail-eyebrow">
      {{ plugin.local_only ? "本地插件" : "目录插件" }}
      <Badge
        :variant="plugin.status === 'installed' ? 'secondary' : 'outline'"
        class="marketplace-detail-status"
      >
        {{ marketplaceStatusLabel(plugin.status) }}
      </Badge>
    </div>
    <h2 :id="`plugin-detail-${plugin.id}`" class="marketplace-detail-heading">
      {{ plugin.name }}
    </h2>
    <p class="marketplace-detail-id">{{ plugin.id }}</p>
    <p class="marketplace-detail-description">{{ plugin.description }}</p>

    <section v-if="plugin.tags.length" class="marketplace-detail-section">
      <h3 class="marketplace-section-label">标签</h3>
      <div class="marketplace-tag-row">
        <span v-for="tag in plugin.tags" :key="tag" class="marketplace-tag">{{
          tag
        }}</span>
      </div>
    </section>

    <section class="marketplace-detail-section">
      <h3 class="marketplace-section-label">目录信息</h3>
      <dl class="marketplace-detail-list">
        <div>
          <dt>仓库</dt>
          <dd>
            <a
              v-if="plugin.repository_url"
              :href="plugin.repository_url"
              target="_blank"
              rel="noreferrer"
              class="marketplace-repository-link"
            >
              {{ plugin.repository_url }}
              <ExternalLink class="h-3 w-3" aria-hidden="true" />
            </a>
            <span v-else>目录未提供</span>
          </dd>
        </div>
        <div>
          <dt>安装来源</dt>
          <dd class="font-mono">{{ marketplaceInstallSpec(plugin) }}</dd>
        </div>
        <div>
          <dt>分类</dt>
          <dd>{{ plugin.category ?? "目录未提供" }}</dd>
        </div>
        <div>
          <dt>GitHub Stars</dt>
          <dd>
            {{
              plugin.popularity.github_stars?.toLocaleString() ?? "Stars 未提供"
            }}
          </dd>
        </div>
        <div>
          <dt>加入目录</dt>
          <dd>{{ marketplaceDate(plugin.source_updated_at) }}</dd>
        </div>
        <div>
          <dt>静态校验</dt>
          <dd>{{ marketplaceDate(plugin.validated_at) }}</dd>
        </div>
      </dl>
    </section>

    <section
      v-if="pendingAction === 'install'"
      class="marketplace-operation-panel marketplace-operation-install"
      aria-label="确认安装"
    >
      <p class="marketplace-operation-heading">
        确认安装到 <span class="font-mono">{{ profile }}</span>
      </p>
      <p class="marketplace-operation-copy">
        Launcher 只会使用下方的固定来源，不会执行目录返回的展示命令。
      </p>
      <code class="marketplace-command-preview"
        >dsh plugin --profile {{ profile }} add
        {{ marketplaceInstallSpec(plugin) }}</code
      >
    </section>
    <section
      v-else-if="pendingAction === 'remove'"
      class="marketplace-operation-panel marketplace-operation-remove"
      aria-label="确认卸载"
    >
      <p class="marketplace-operation-heading">
        从 <span class="font-mono">{{ profile }}</span> 移除插件
      </p>
      <p class="marketplace-operation-copy">
        这会使用本地 profile 中已解析的安装 spec，不会影响其他 profile。
      </p>
      <code class="marketplace-command-preview"
        >dsh plugin --profile {{ profile }} remove
        {{ plugin.installed_source }}</code
      >
    </section>

    <aside class="marketplace-risk-note">
      <strong>目录校验插件结构，不代表代码安全。</strong>
      安装前请确认仓库来源；插件可在 dsh 中执行其声明的能力。
    </aside>
  </div>
</template>

<style scoped>
.marketplace-detail-eyebrow {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  color: hsl(var(--muted-foreground));
  font-size: 10px;
  font-weight: 650;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
.marketplace-detail-status {
  padding: 1px 6px;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0;
  text-transform: none;
}
.marketplace-detail-heading {
  margin: 0;
  font-size: 18px;
  font-weight: 640;
  letter-spacing: -0.022em;
}
.marketplace-detail-id {
  margin: 4px 0 17px;
  color: hsl(var(--muted-foreground));
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 11px;
}
.marketplace-detail-description {
  margin: 0;
  color: hsl(var(--muted-foreground));
  font-size: 13px;
  line-height: 1.65;
}
.marketplace-detail-section {
  margin-top: 22px;
}
.marketplace-section-label {
  margin: 0 0 8px;
  color: hsl(var(--muted-foreground));
  font-size: 10px;
  font-weight: 650;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
.marketplace-tag-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.marketplace-tag {
  padding: 3px 7px;
  border-radius: 5px;
  background: hsl(var(--secondary));
  color: hsl(var(--muted-foreground));
  font-size: 11px;
}
.marketplace-detail-list {
  margin: 0;
  border-top: 1px solid hsl(var(--border));
}
.marketplace-detail-list > div {
  display: grid;
  grid-template-columns: 94px minmax(0, 1fr);
  gap: 12px;
  padding: 10px 0;
  border-bottom: 1px solid hsl(var(--border));
}
.marketplace-detail-list dt {
  color: hsl(var(--muted-foreground));
  font-size: 11px;
}
.marketplace-detail-list dd {
  min-width: 0;
  margin: 0;
  overflow-wrap: anywhere;
  color: hsl(var(--muted-foreground));
  font-size: 11px;
}
.marketplace-repository-link {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: hsl(var(--foreground));
  text-decoration: none;
}
.marketplace-repository-link:hover {
  text-decoration: underline;
  text-underline-offset: 3px;
}
.marketplace-risk-note {
  margin-top: 20px;
  padding: 11px 12px;
  border: 1px solid hsl(var(--border));
  border-radius: 8px;
  background: hsl(var(--secondary));
  color: hsl(var(--muted-foreground));
  font-size: 11px;
  line-height: 1.6;
}
.marketplace-risk-note strong {
  color: hsl(var(--foreground));
  font-weight: 600;
}
.marketplace-operation-panel {
  margin-top: 18px;
  padding: 13px;
  border: 1px solid hsl(var(--border));
  border-radius: 9px;
  background: hsl(var(--secondary));
}
.marketplace-operation-install {
  border-color: hsl(var(--ring));
}
.marketplace-operation-remove {
  border-color: hsl(var(--destructive));
}
.marketplace-operation-heading {
  margin: 0;
  font-size: 12px;
  font-weight: 620;
}
.marketplace-operation-copy {
  margin: 5px 0 0;
  color: hsl(var(--muted-foreground));
  font-size: 11px;
  line-height: 1.55;
}
.marketplace-command-preview {
  display: block;
  margin-top: 10px;
  padding: 8px 9px;
  overflow-x: auto;
  border: 1px solid hsl(var(--border));
  border-radius: 6px;
  background: hsl(var(--background));
  color: hsl(var(--muted-foreground));
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 10px;
  white-space: nowrap;
}
@media (max-width: 620px) {
  .marketplace-detail-list > div {
    grid-template-columns: 82px minmax(0, 1fr);
  }
}
</style>
