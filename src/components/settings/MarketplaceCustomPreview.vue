<script setup lang="ts">
import type { MarketplaceCustomInstallPreview } from "@/lib/tauri";

interface Props {
  preview: MarketplaceCustomInstallPreview;
}

defineProps<Props>();
</script>

<template>
  <div>
    <div class="marketplace-detail-eyebrow">自定义安装</div>
    <h2 class="marketplace-detail-heading">自定义来源，尚未执行</h2>
    <p class="marketplace-detail-description">
      此来源未经过市场目录校验。Launcher 只会使用后端已解析的字段重新构造 dsh
      参数。
    </p>
    <dl class="marketplace-detail-list mt-5">
      <div>
        <dt>目标 profile</dt>
        <dd class="font-mono">{{ preview.profile }}</dd>
      </div>
      <div>
        <dt>来源</dt>
        <dd class="font-mono">{{ preview.source }}</dd>
      </div>
      <div>
        <dt>执行者</dt>
        <dd>Launcher 托管的 dsh {{ preview.dsh_version }}</dd>
      </div>
    </dl>
    <aside class="marketplace-risk-note">
      <strong>请确认来源。</strong> 该命令不会被交给
      shell，但其来源不在当前目录中。
    </aside>
  </div>
</template>

<style scoped>
.marketplace-detail-eyebrow {
  margin-bottom: 8px;
  color: hsl(var(--muted-foreground));
  font-size: 10px;
  font-weight: 650;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
.marketplace-detail-heading {
  margin: 0;
  font-size: 18px;
  font-weight: 640;
  letter-spacing: -0.022em;
}
.marketplace-detail-description {
  margin: 0;
  color: hsl(var(--muted-foreground));
  font-size: 13px;
  line-height: 1.65;
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
@media (max-width: 620px) {
  .marketplace-detail-list > div {
    grid-template-columns: 82px minmax(0, 1fr);
  }
}
</style>
