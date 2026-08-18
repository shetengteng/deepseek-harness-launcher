<script setup lang="ts">
import { AlertTriangle, RefreshCw } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import { useDshUpdate } from "@/composables/useDshUpdate";
import { useI18n } from "@/lib/i18n";

const emit = defineEmits<{ "open-settings": [] }>();
const { t } = useI18n();

const {
  updateDialogOpen,
  updateDialogState,
  updateProgress,
  updateStageMessage,
  updateCurrentVersion,
  updateTargetVersion,
  updateError,
  nodeUpgrade,
  startDshUpdate,
  confirmNodeUpgrade,
  cancelDshUpdate,
  closeUpdateDialog,
} = useDshUpdate();

function openUpdateSettings(): void {
  closeUpdateDialog();
  emit("open-settings");
}
</script>

<template>
  <Dialog :open="updateDialogOpen">
    <DialogContent
      class="sm:max-w-[420px] [&>button]:hidden"
      @escape-key-down.prevent
      @pointer-down-outside.prevent
    >
      <template v-if="updateDialogState === 'failed'">
        <DialogHeader>
          <DialogTitle class="flex items-center gap-2">
            <AlertTriangle class="h-5 w-5 text-warning" />
            {{ t("update.failedTitle") }}
          </DialogTitle>
          <DialogDescription>
            {{ t("update.failedDescription") }}
          </DialogDescription>
        </DialogHeader>

        <div
          class="rounded-md border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
          role="alert"
        >
          <div class="mb-1 text-xs font-medium">{{ t("update.failed") }}</div>
          <p class="break-words text-xs leading-5 text-foreground/80">
            {{ updateError }}
          </p>
        </div>

        <DialogFooter class="gap-2 sm:gap-2">
          <Button variant="ghost" @click="closeUpdateDialog">{{ t("error.close") }}</Button>
          <Button variant="outline" @click="openUpdateSettings">{{ t("update.changeSource") }}</Button>
          <Button @click="startDshUpdate">
            <RefreshCw class="h-4 w-4" />
            {{ t("error.retry") }}
          </Button>
        </DialogFooter>
      </template>

      <template v-else-if="updateDialogState === 'confirming_node' && nodeUpgrade">
        <DialogHeader>
          <DialogTitle class="flex items-center gap-2">
            <AlertTriangle class="h-5 w-5 text-warning" />
            {{ t("update.nodeRequired") }}
          </DialogTitle>
          <DialogDescription>
            {{ t("update.nodeRequiredDescription", { dshVersion: nodeUpgrade.dsh_version, requiredVersion: nodeUpgrade.engines_node, currentVersion: nodeUpgrade.current_node }) }}
          </DialogDescription>
        </DialogHeader>

        <div class="space-y-2 rounded-md border bg-muted/30 px-3 py-2 text-sm">
          <div class="flex items-center justify-between gap-4">
            <span class="text-muted-foreground">{{ t("update.currentNode") }}</span>
            <span class="font-mono text-xs">{{ nodeUpgrade.current_node }}</span>
          </div>
          <div class="flex items-center justify-between gap-4">
            <span class="text-muted-foreground">{{ t("update.willInstall") }}</span>
            <span class="font-mono text-xs text-info">
              {{ nodeUpgrade.suggested_node }}
            </span>
          </div>
        </div>

        <DialogFooter class="gap-2 sm:gap-2">
          <Button variant="outline" @click="cancelDshUpdate">{{ t("update.cancel") }}</Button>
          <Button @click="confirmNodeUpgrade">{{ t("update.confirm") }}</Button>
        </DialogFooter>
      </template>

      <template v-else>
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
            <span class="font-mono text-xs">{{
              updateCurrentVersion ?? "—"
            }}</span>
          </div>
          <div class="flex items-center justify-between gap-4">
            <span class="text-muted-foreground">{{ t("update.target") }}</span>
            <span class="font-mono text-xs text-info">
              {{ updateTargetVersion ?? "—" }}
            </span>
          </div>
        </div>

        <Progress :model-value="updateProgress" class="h-2 [&>div]:bg-info" />
        <div
          class="-mt-2 flex items-center justify-between gap-4 font-mono text-[11px] text-muted-foreground"
          aria-live="polite"
          role="status"
        >
          <span>{{ updateStageMessage }}</span>
          <span>{{ t("update.wait") }}</span>
        </div>

        <DialogFooter>
          <Button
            variant="outline"
            :disabled="
              updateDialogState !== 'installing' &&
              updateDialogState !== 'upgrading_node'
            "
            @click="cancelDshUpdate"
          >
            {{ updateDialogState === "cancelling" ? t("update.cancelling") : t("common.cancel") }}
          </Button>
        </DialogFooter>
      </template>
    </DialogContent>
  </Dialog>
</template>
