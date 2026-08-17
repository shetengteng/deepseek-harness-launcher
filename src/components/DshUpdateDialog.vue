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

const emit = defineEmits<{ "open-settings": [] }>();

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
            dsh 更新失败
          </DialogTitle>
          <DialogDescription>
            新版本没有安装成功，当前版本仍然可以继续使用。
          </DialogDescription>
        </DialogHeader>

        <div
          class="rounded-md border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive"
          role="alert"
        >
          <div class="mb-1 text-xs font-medium">更新失败</div>
          <p class="break-words text-xs leading-5 text-foreground/80">
            {{ updateError }}
          </p>
        </div>

        <DialogFooter class="gap-2 sm:gap-2">
          <Button variant="ghost" @click="closeUpdateDialog">关闭</Button>
          <Button variant="outline" @click="openUpdateSettings">更换源</Button>
          <Button @click="startDshUpdate">
            <RefreshCw class="h-4 w-4" />
            重试
          </Button>
        </DialogFooter>
      </template>

      <template v-else-if="updateDialogState === 'confirming_node' && nodeUpgrade">
        <DialogHeader>
          <DialogTitle class="flex items-center gap-2">
            <AlertTriangle class="h-5 w-5 text-warning" />
            需要升级 Node
          </DialogTitle>
          <DialogDescription>
            dsh {{ nodeUpgrade.dsh_version }} 需要 Node
            {{ nodeUpgrade.engines_node }}，当前为
            {{ nodeUpgrade.current_node }}。
          </DialogDescription>
        </DialogHeader>

        <div class="space-y-2 rounded-md border bg-muted/30 px-3 py-2 text-sm">
          <div class="flex items-center justify-between gap-4">
            <span class="text-muted-foreground">当前 Node</span>
            <span class="font-mono text-xs">{{ nodeUpgrade.current_node }}</span>
          </div>
          <div class="flex items-center justify-between gap-4">
            <span class="text-muted-foreground">将安装</span>
            <span class="font-mono text-xs text-info">
              {{ nodeUpgrade.suggested_node }}
            </span>
          </div>
        </div>

        <DialogFooter class="gap-2 sm:gap-2">
          <Button variant="outline" @click="cancelDshUpdate">取消更新</Button>
          <Button @click="confirmNodeUpgrade">确认升级并继续</Button>
        </DialogFooter>
      </template>

      <template v-else>
        <DialogHeader>
          <DialogTitle class="flex items-center gap-2">
            <RefreshCw class="h-5 w-5 animate-spin text-info" />
            正在更新 dsh
          </DialogTitle>
          <DialogDescription>
            正在下载并校验新版本，当前会话继续使用旧版本。
          </DialogDescription>
        </DialogHeader>

        <div class="space-y-2 rounded-md border bg-muted/30 px-3 py-2 text-sm">
          <div class="flex items-center justify-between gap-4">
            <span class="text-muted-foreground">当前</span>
            <span class="font-mono text-xs">{{
              updateCurrentVersion ?? "—"
            }}</span>
          </div>
          <div class="flex items-center justify-between gap-4">
            <span class="text-muted-foreground">目标</span>
            <span class="font-mono text-xs text-info">
              {{ updateTargetVersion ?? "—" }}
            </span>
          </div>
        </div>

        <Progress :model-value="updateProgress" class="h-2 [&>div]:bg-info" />
        <div
          class="-mt-2 flex items-center justify-between gap-4 text-xs text-muted-foreground"
          aria-live="polite"
          role="status"
        >
          <span>{{ updateStageMessage }}</span>
          <span>请稍候</span>
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
            {{ updateDialogState === "cancelling" ? "正在取消…" : "取消" }}
          </Button>
        </DialogFooter>
      </template>
    </DialogContent>
  </Dialog>
</template>
