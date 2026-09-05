<script setup lang="ts">
import { RefreshCw } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import { useI18n } from "@/lib/i18n";

defineProps<{
  progress: number;
  stageMessage: string;
  currentVersion: string | null;
  targetVersion: string | null;
  cancelling: boolean;
  cancelDisabled: boolean;
}>();
defineEmits<{ cancel: [] }>();
const { t } = useI18n();
</script>

<template>
  <DialogHeader>
    <DialogTitle class="flex items-center gap-2">
      <RefreshCw class="h-5 w-5 animate-spin text-info" />
      {{ t("update.updating") }}
    </DialogTitle>
    <DialogDescription>
      {{ t("update.updatingDescription") }}
    </DialogDescription>
  </DialogHeader>

  <div class="space-y-2 rounded-md border bg-muted/30 px-3 py-2 text-sm">
    <div class="flex items-center justify-between gap-4">
      <span class="text-muted-foreground">{{ t("update.current") }}</span>
      <span class="font-mono text-xs">{{ currentVersion ?? "—" }}</span>
    </div>
    <div class="flex items-center justify-between gap-4">
      <span class="text-muted-foreground">{{ t("update.target") }}</span>
      <span class="font-mono text-xs text-info">{{ targetVersion ?? "—" }}</span>
    </div>
  </div>

  <Progress :model-value="progress" class="h-2 [&>div]:bg-info" />
  <div
    class="-mt-2 flex items-center justify-between gap-4 font-mono text-[11px] text-muted-foreground"
    aria-live="polite"
    role="status"
    data-testid="dsh-update-progress-status"
  >
    <span>{{ stageMessage }}</span>
    <span>{{ t("update.wait") }}</span>
  </div>

  <DialogFooter>
    <Button variant="outline" :disabled="cancelDisabled" @click="$emit('cancel')">
      {{ cancelling ? t("update.cancelling") : t("common.cancel") }}
    </Button>
  </DialogFooter>
</template>
