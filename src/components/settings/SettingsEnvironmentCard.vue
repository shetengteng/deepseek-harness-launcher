<script setup lang="ts">
import { computed } from "vue";
import { Download, RefreshCw } from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DshStateSnapshot, LatestDshVersion } from "@/lib/tauri";
import { useI18n } from "@/lib/i18n";

const props = defineProps<{
  dshState: DshStateSnapshot;
  nodeVersion: string | null;
  hostOrigin: string | null;
  latestVersion: LatestDshVersion | null;
  refreshing: boolean;
  upgrading: boolean;
  error: string | null;
  nodeUpdateLoading: boolean;
  nodeUpdateError: string | null;
}>();
const { t } = useI18n();

defineEmits<{ refresh: []; install: []; updateNode: [] }>();

const updateAvailable = computed(
  () =>
    props.latestVersion !== null &&
    props.dshState.current !== props.latestVersion.latest_version,
);

const hostAddress = computed(() => {
  if (!props.hostOrigin) return null;
  try {
    return new URL(props.hostOrigin).host;
  } catch {
    return props.hostOrigin;
  }
});
</script>

<template>
  <Card>
    <CardHeader><CardTitle class="text-base">{{ t("environment.title") }}</CardTitle></CardHeader>
    <CardContent class="space-y-3">
      <div class="flex items-center justify-between gap-4">
        <div class="min-w-0">
          <div class="text-sm">{{ t("environment.dshVersion") }}</div>
          <p
            class="min-h-5 text-xs text-muted-foreground"
            data-testid="dsh-update-status"
          >
            <span v-if="error" class="text-destructive">{{ error }}</span>
            <template v-else-if="updateAvailable">
              {{ t("environment.updateAvailable") }}<span class="font-mono">{{
                latestVersion?.latest_version
              }}</span></template
            >
            <span v-else role="status">{{ t("environment.upToDate") }}</span>
          </p>
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <Badge v-if="dshState.current" variant="default">{{
            dshState.current
          }}</Badge
          ><span v-else class="text-sm text-muted-foreground">{{ t("environment.notInstalled") }}</span>
          <Button
            :variant="updateAvailable ? 'default' : 'outline'"
            size="xs"
            class="rounded-full"
            :disabled="refreshing || upgrading"
            @click="updateAvailable ? $emit('install') : $emit('refresh')"
            ><Download v-if="updateAvailable" class="mr-2 h-4 w-4" /><RefreshCw
              v-else
              :class="['mr-2 h-4 w-4', refreshing && 'animate-spin']"
            />{{
              updateAvailable
                ? upgrading
                  ? t("environment.installing")
                  : t("environment.installUpdate")
                : refreshing
                  ? t("environment.refreshing")
                  : t("environment.refresh")
            }}</Button
          >
        </div>
      </div>
      <div class="flex items-center justify-between gap-4">
        <div class="min-w-0">
          <div class="text-sm">{{ t("environment.nodeVersion") }}</div>
          <div
            class="text-xs text-muted-foreground"
            data-testid="node-update-status"
          >
            <span v-if="nodeUpdateError" class="text-destructive">{{
              nodeUpdateError
            }}</span>
            <span v-else>{{ t("environment.nodeHint") }}</span>
          </div>
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <span class="font-mono text-sm">{{ nodeVersion ?? t("environment.notInstalled") }}</span>
          <Button
            variant="outline"
            size="xs"
            class="rounded-full"
            :disabled="nodeVersion === null || nodeUpdateLoading"
            @click="$emit('updateNode')"
          >
            <Download class="mr-2 h-4 w-4" />
            {{ nodeUpdateLoading ? t("environment.preparing") : t("environment.updateNode") }}
          </Button>
        </div>
      </div>
      <div class="flex items-center justify-between gap-4">
        <div>
          <div class="text-sm">{{ t("environment.hostAddress") }}</div>
          <div class="text-xs text-muted-foreground">
            {{ t("environment.hostDescription") }}
          </div>
        </div>
        <span class="font-mono text-sm">{{ hostAddress ?? t("environment.notRunning") }}</span>
      </div>
    </CardContent>
  </Card>
</template>
