<script setup lang="ts">
// 错误对话框。对应设计 §M1.5 + page-flow-analysis.md §3.4。
// 根据 `lastFailedAction` 决定"重试"按钮的文案与行为。

import { computed } from "vue";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import type { LauncherErrorPayload } from "@/lib/tauri";

type LastAction = "installNode" | "installDsh" | "startHost" | "shutdownHost" | null;

const props = defineProps<{
  /** 错误详情。非空时对话框打开。 */
  error: LauncherErrorPayload | null;
  /** 上次失败的操作，决定"重试"按钮文案。null 时隐藏重试按钮。 */
  lastFailedAction?: LastAction;
}>();

const emit = defineEmits<{
  /** 用户点击"关闭"。调用方负责 `resetError` 恢复到错误前 phase。 */
  (e: "dismiss"): void;
  /** 用户点击"重试"。调用方负责 `retryLastAction` 重新执行失败的操作。 */
  (e: "retry"): void;
}>();

const open = computed(() => props.error !== null);

/** 展示文案：优先 `user_message`（Rust 端映射的中文提示 + 可操作建议，PR-019），
 * 缺失时回退到 `message`。 */
const displayMessage = computed(() => {
  if (!props.error) return "";
  return props.error.user_message ?? props.error.message;
});

/** 是否显示技术详情折叠区（user_message 存在且与 message 不同时展示原始信息）。 */
const hasTechnicalDetail = computed(() => {
  if (!props.error) return false;
  return Boolean(props.error.user_message) && props.error.user_message !== props.error.message;
});

/** 重试按钮文案，根据失败的操作类型决定。 */
const retryLabel = computed(() => {
  switch (props.lastFailedAction) {
    case "installNode":
      return "重试安装 Node";
    case "installDsh":
      return "重试安装 Harness";
    case "startHost":
      return "重试启动";
    case "shutdownHost":
      return "重试关闭";
    default:
      return "重试";
  }
});

/** 是否显示重试按钮。没有 lastFailedAction 时隐藏（如 loadMirrors 失败）。 */
const showRetry = computed(() => props.lastFailedAction !== null);

function onRetry() {
  emit("retry");
}

function onDismiss() {
  emit("dismiss");
}
</script>

<template>
  <Dialog :open="open" @update:open="(v) => !v && onDismiss()">
    <DialogContent class="sm:max-w-[520px]">
      <DialogHeader>
        <DialogTitle>操作失败</DialogTitle>
        <DialogDescription
          v-if="props.error"
          class="break-words whitespace-pre-wrap"
        >
          {{ displayMessage }}
        </DialogDescription>
      </DialogHeader>
      <div
        v-if="hasTechnicalDetail"
        class="rounded border bg-muted/30 p-2 text-xs font-mono max-h-[120px] overflow-auto"
      >
        <pre class="whitespace-pre-wrap break-words">{{ props.error?.message }}</pre>
      </div>
      <div
        v-if="props.error?.data"
        class="rounded border bg-muted/30 p-2 text-xs font-mono max-h-[200px] overflow-auto"
      >
        <pre class="whitespace-pre-wrap break-words">{{ JSON.stringify(props.error.data, null, 2) }}</pre>
      </div>
      <DialogFooter>
        <Button variant="outline" @click="onDismiss">关闭</Button>
        <Button v-if="showRetry" variant="default" @click="onRetry">
          {{ retryLabel }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
