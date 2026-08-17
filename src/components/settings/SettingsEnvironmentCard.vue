<script setup lang="ts">
import { computed } from "vue";
import { Download, RefreshCw } from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DshStateSnapshot, LatestDshVersion } from "@/lib/tauri";

const props = defineProps<{
  dshState: DshStateSnapshot;
  nodeVersion: string | null;
  latestVersion: LatestDshVersion | null;
  refreshing: boolean;
  upgrading: boolean;
  error: string | null;
}>();

defineEmits<{ refresh: []; install: [] }>();

const updateAvailable = computed(
  () =>
    props.latestVersion !== null &&
    props.dshState.current !== props.latestVersion.latest_version,
);
</script>

<template>
  <Card>
    <CardHeader><CardTitle class="text-base">运行环境</CardTitle></CardHeader>
    <CardContent class="space-y-3">
      <div class="space-y-1">
        <div class="flex min-h-7 items-center justify-between gap-4">
          <div class="text-sm">正在使用的 DeepSeek Harness 版本</div>
          <div class="flex shrink-0 items-center gap-2">
            <Badge v-if="dshState.current" variant="default">{{
              dshState.current
            }}</Badge
            ><span v-else class="text-sm text-muted-foreground">尚未安装</span>
            <Button
              :variant="updateAvailable ? 'default' : 'outline'"
              size="xs"
              :disabled="refreshing || upgrading"
              @click="updateAvailable ? $emit('install') : $emit('refresh')"
              ><Download
                v-if="updateAvailable"
                class="mr-2 h-4 w-4"
              /><RefreshCw
                v-else
                :class="['mr-2 h-4 w-4', refreshing && 'animate-spin']"
              />{{
                updateAvailable
                  ? upgrading
                    ? "安装中…"
                    : "安装新版本"
                  : refreshing
                    ? "刷新中…"
                    : "刷新"
              }}</Button
            >
          </div>
        </div>
        <div
          class="flex min-h-7 items-center"
          data-testid="dsh-update-status"
        >
          <p v-if="error" class="text-sm text-destructive">{{ error }}</p>
          <p v-else-if="updateAvailable" class="text-sm text-muted-foreground">
            可更新版本：<span class="font-mono">{{
              latestVersion?.latest_version
            }}</span>
          </p>
          <p v-else class="text-sm text-muted-foreground" role="status">
            已是最新版本
          </p>
        </div>
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
