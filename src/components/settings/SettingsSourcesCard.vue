<script setup lang="ts">
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { MirrorInfo } from "@/lib/tauri";

defineProps<{
  nodeMirrors: MirrorInfo[];
  nodeMirror: string;
  registry: string;
  error: string | null;
}>();
defineEmits<{
  setNodeMirror: [value: unknown];
  setRegistry: [value: unknown];
}>();

function nodeMirrorLabel(mirror: MirrorInfo): string {
  return mirror.id === "tuna" ? "tsinghua（清华大学）" : mirror.id;
}
</script>

<template>
  <Card>
    <CardHeader><CardTitle class="text-base">下载来源</CardTitle></CardHeader>
    <CardContent class="space-y-4">
      <div class="flex items-center justify-between gap-4 py-3">
        <div class="min-w-0">
          <Label>Node.js 下载源</Label>
          <p class="text-xs text-muted-foreground">
            下次下载或更新 Node.js 时使用
          </p>
        </div>
        <Select
          :model-value="nodeMirror"
          @update:model-value="$emit('setNodeMirror', $event)"
          ><SelectTrigger class="w-44 shrink-0"
            ><SelectValue placeholder="选择下载源" /></SelectTrigger
          ><SelectContent
            ><SelectItem
              v-for="mirror in nodeMirrors"
              :key="mirror.id"
              :value="mirror.base_url"
              >{{ nodeMirrorLabel(mirror) }}</SelectItem
            ></SelectContent
          ></Select
        >
      </div>
      <div class="flex items-center justify-between gap-4 py-3">
        <div class="min-w-0">
          <Label>npm 下载源</Label>
          <p class="text-xs text-muted-foreground">
            下次安装或更新 DeepSeek Harness 时使用
          </p>
        </div>
        <Select
          :model-value="registry"
          @update:model-value="$emit('setRegistry', $event)"
          ><SelectTrigger class="w-44 shrink-0"
            ><SelectValue placeholder="选择下载源" /></SelectTrigger
          ><SelectContent
            ><SelectItem value="https://registry.npmmirror.com"
              >npmmirror.com</SelectItem
            ><SelectItem value="https://registry.npmjs.org"
              >npmjs.com（官方）</SelectItem
            ></SelectContent
          ></Select
        >
      </div>
      <p v-if="error" class="text-xs text-destructive">{{ error }}</p>
    </CardContent>
  </Card>
</template>
