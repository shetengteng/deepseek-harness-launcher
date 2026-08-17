<script setup lang="ts">
import { computed } from "vue";
import { Download, RefreshCw } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DshStateSnapshot, LatestDshVersion } from "@/lib/tauri";

const props = defineProps<{
  dshState: DshStateSnapshot;
  latestVersion: LatestDshVersion | null;
  refreshing: boolean;
  upgrading: boolean;
  error: string | null;
}>();

defineEmits<{ refresh: []; install: [] }>();

const latestIsCurrent = computed(
  () =>
    props.dshState.current !== null &&
    props.dshState.current === props.latestVersion?.latest_version,
);
</script>

<template>
  <Card>
    <CardHeader
      ><CardTitle class="text-base"
        >DeepSeek Harness 更新</CardTitle
      ></CardHeader
    >
    <CardContent class="space-y-3">
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <div class="text-sm font-medium">当前最新版本</div>
          <p class="mt-1 font-mono text-xs text-muted-foreground">
            {{ latestVersion?.latest_version ?? "正在读取…" }}
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          :disabled="refreshing || upgrading"
          @click="$emit('refresh')"
          ><RefreshCw
            :class="['mr-2 h-4 w-4', refreshing && 'animate-spin']"
          />刷新</Button
        >
      </div>
      <p class="border-t pt-3 text-xs text-muted-foreground">
        更新只会在你点击按钮后下载。安装失败或新版本无法启动时，将继续保留当前版本。
      </p>
      <div class="flex justify-end">
        <Button
          size="sm"
          :disabled="!latestVersion || upgrading || latestIsCurrent"
          @click="$emit('install')"
          ><Download class="mr-2 h-4 w-4" />{{
            latestIsCurrent
              ? "已是最新版本"
              : upgrading
                ? "更新中…"
                : "更新到最新版本"
          }}</Button
        >
      </div>
      <p v-if="error" class="text-sm text-destructive">{{ error }}</p>
    </CardContent>
  </Card>
</template>
