<script setup lang="ts">
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DshStateSnapshot } from "@/lib/tauri";

defineProps<{ dshState: DshStateSnapshot; nodeVersion: string | null }>();
</script>

<template>
  <Card>
    <CardHeader><CardTitle class="text-base">运行环境</CardTitle></CardHeader>
    <CardContent class="space-y-3">
      <div class="flex items-center justify-between gap-4">
        <div>
          <div class="text-sm">正在使用的 DeepSeek Harness 版本</div>
          <div class="text-xs text-muted-foreground">
            当前启动的 DeepSeek Harness
          </div>
        </div>
        <Badge v-if="dshState.current" variant="default">{{
          dshState.current
        }}</Badge
        ><span v-else class="text-sm text-muted-foreground">尚未安装</span>
      </div>
      <div class="flex items-center justify-between gap-4">
        <div>
          <div class="text-sm">可恢复的版本</div>
          <div class="text-xs text-muted-foreground">
            如果新版本无法启动，应用会切换回这个版本
          </div>
        </div>
        <Badge v-if="dshState.known_good" variant="secondary">{{
          dshState.known_good
        }}</Badge
        ><span v-else class="text-sm text-muted-foreground">暂时没有</span>
      </div>
      <div class="flex items-center justify-between gap-4">
        <div>
          <div class="text-sm">Node.js 版本</div>
          <div class="text-xs text-muted-foreground">
            由应用自动管理，无需手动安装
          </div>
        </div>
        <span class="font-mono text-sm">{{ nodeVersion ?? "尚未安装" }}</span>
      </div>
    </CardContent>
  </Card>
</template>
