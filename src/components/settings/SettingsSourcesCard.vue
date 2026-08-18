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
import { useI18n } from "@/lib/i18n";

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
const { t } = useI18n();

function nodeMirrorLabel(mirror: MirrorInfo): string {
  return mirror.id === "tuna" ? t("sources.tsinghua") : mirror.id;
}
</script>

<template>
  <Card>
    <CardHeader><CardTitle class="text-base">{{ t("sources.title") }}</CardTitle></CardHeader>
    <CardContent class="space-y-3">
      <div class="flex items-center justify-between gap-4">
        <div class="min-w-0">
          <Label>{{ t("sources.node") }}</Label>
          <p class="text-xs text-muted-foreground">
            {{ t("sources.nodeDescription") }}
          </p>
        </div>
        <Select
          :model-value="nodeMirror"
          @update:model-value="$emit('setNodeMirror', $event)"
          ><SelectTrigger class="h-7 w-44 shrink-0 text-xs"
            ><SelectValue :placeholder="t('sources.select')" /></SelectTrigger
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
      <div class="flex items-center justify-between gap-4">
        <div class="min-w-0">
          <Label>{{ t("sources.npm") }}</Label>
          <p class="text-xs text-muted-foreground">
            {{ t("sources.npmDescription") }}
          </p>
        </div>
        <Select
          :model-value="registry"
          @update:model-value="$emit('setRegistry', $event)"
          ><SelectTrigger class="h-7 w-44 shrink-0 text-xs"
            ><SelectValue :placeholder="t('sources.select')" /></SelectTrigger
          ><SelectContent
            ><SelectItem value="https://registry.npmmirror.com"
              >npmmirror.com</SelectItem
            ><SelectItem value="https://registry.npmjs.org"
              >{{ t("sources.official") }}</SelectItem
            ></SelectContent
          ></Select
        >
      </div>
      <p v-if="error" class="text-xs text-destructive">{{ error }}</p>
    </CardContent>
  </Card>
</template>
