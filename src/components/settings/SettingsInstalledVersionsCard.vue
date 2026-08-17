<script setup lang="ts">
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DshStateSnapshot } from "@/lib/tauri";

defineProps<{ dshState: DshStateSnapshot }>();

function statusBadgeVariant(status: string) {
  switch (status) {
    case "verified":
      return undefined;
    case "broken":
      return "destructive" as const;
    default:
      return "outline" as const;
  }
}

function statusLabel(status: string) {
  switch (status) {
    case "verified":
      return "可用";
    case "broken":
      return "无法使用";
    default:
      return "状态未知";
  }
}
</script>

<template>
  <Card v-if="dshState.installed.length > 0">
    <CardHeader
      ><CardTitle class="text-base">已下载的版本</CardTitle></CardHeader
    >
    <CardContent
      ><div class="space-y-2">
        <div
          v-for="installed in dshState.installed"
          :key="installed.version"
          class="flex items-center justify-between py-1"
        >
          <span class="text-sm font-mono">{{ installed.version }}</span
          ><Badge :variant="statusBadgeVariant(installed.status)">{{
            statusLabel(installed.status)
          }}</Badge>
        </div>
      </div></CardContent
    >
  </Card>
</template>
