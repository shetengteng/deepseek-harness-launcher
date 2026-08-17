<script setup lang="ts">
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

defineProps<{ open: boolean }>();

const emit = defineEmits<{
  (e: "close"): void;
}>();
</script>

<template>
  <Dialog :open="open" @update:open="(value) => !value && emit('close')">
    <DialogContent class="sm:max-w-[420px]">
      <DialogHeader>
        <DialogTitle>DeepSeek Harness Launcher</DialogTitle>
        <DialogDescription>
          管理 DeepSeek Harness 的 Node 运行时与版本切换。
        </DialogDescription>
      </DialogHeader>
      <dl class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-sm">
        <dt class="text-muted-foreground">壳子版本</dt>
        <dd>v0.1.0</dd>
        <dt class="text-muted-foreground">技术栈</dt>
        <dd>Tauri · Vue · Rust</dd>
      </dl>
      <DialogFooter>
        <Button @click="emit('close')">关闭</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
