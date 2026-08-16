<script setup lang="ts">
// 崩溃恢复弹窗。对应设计 §5.5 / PR-017。
// Host 崩溃达到重试上限（或自动重启失败）后弹出。
// 选项：重试（清零计数器重启）/ 回滚 known_good / 忽略。

import { computed } from "vue";
import { AlertTriangle, RotateCcw, Undo2, X } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { CrashLimitPayload } from "@/lib/tauri";

const props = defineProps<{
  /** 崩溃上限 payload。非空时弹窗打开。 */
  crash: CrashLimitPayload | null;
  /** 重试/回滚操作进行中。 */
  recovering: boolean;
}>();

const emit = defineEmits<{
  /** 用户点"重试"：清零计数器后重启 Host。 */
  (e: "retry"): void;
  /** 用户点"回滚"：切到 known_good 后重启。 */
  (e: "rollback"): void;
  /** 用户点"忽略"：关闭弹窗不重启。 */
  (e: "dismiss"): void;
}>();

const open = computed(() => props.crash !== null);

/** 是否显示"回滚"按钮：仅当存在 known_good 稳定版本。 */
const canRollback = computed(() => props.crash?.known_good != null);

/** 退出详情描述。 */
const exitDetail = computed(() => {
  if (!props.crash) return "";
  const parts: string[] = [];
  if (props.crash.exit_code !== null) parts.push(`退出码 ${props.crash.exit_code}`);
  if (props.crash.exit_signal !== null) parts.push(`信号 ${props.crash.exit_signal}`);
  return parts.length > 0 ? `（${parts.join("，")}）` : "";
});
</script>

<template>
  <Dialog :open="open" @update:open="(v) => !v && emit('dismiss')">
    <DialogContent class="sm:max-w-[480px]">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <AlertTriangle class="h-5 w-5 text-destructive" />
          DeepSeek Harness 反复崩溃
        </DialogTitle>
        <DialogDescription class="space-y-2">
          <p v-if="crash">
            Host 在短时间内崩溃了 {{ crash.crash_counter }} 次（自动重试上限
            {{ crash.retry_limit }} 次）{{ exitDetail }}，已停止自动重启。
          </p>
          <p class="text-xs text-muted-foreground">
            可以重试启动；若新版本存在问题，可回滚到上一个稳定版本。
          </p>
        </DialogDescription>
      </DialogHeader>

      <DialogFooter class="gap-2 sm:gap-0">
        <Button variant="outline" :disabled="recovering" @click="emit('dismiss')">
          <X class="h-4 w-4 mr-2" />
          忽略
        </Button>
        <Button
          v-if="canRollback"
          variant="secondary"
          :disabled="recovering"
          @click="emit('rollback')"
        >
          <Undo2 class="h-4 w-4 mr-2" />
          {{ recovering ? "处理中…" : `回滚到 ${crash?.known_good}` }}
        </Button>
        <Button :disabled="recovering" @click="emit('retry')">
          <RotateCcw class="h-4 w-4 mr-2" />
          {{ recovering ? "重启中…" : "重试启动" }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
