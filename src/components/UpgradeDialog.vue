<script setup lang="ts">
// 升级对话框。对应设计 §M3.5。
// 升级准备就绪后弹出，提示用户重启生效。

import { RotateCcw, X } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

defineProps<{
  open: boolean;
  version: string | null;
  upgrading: boolean;
}>();

const emit = defineEmits<{
  (e: "restart"): void;
  (e: "later"): void;
}>();
</script>

<template>
  <Dialog :open="open" @update:open="(val) => !val && emit('later')">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>升级就绪</DialogTitle>
        <DialogDescription>
          <template v-if="version">
            DeepSeek Harness <span class="font-mono font-semibold">{{ version }}</span>
            已安装，重启后生效。
          </template>
          <template v-else>
            新版本已安装，重启后生效。
          </template>
        </DialogDescription>
      </DialogHeader>

      <DialogFooter class="gap-2 sm:gap-0">
        <Button variant="outline" @click="emit('later')">
          <X class="h-4 w-4 mr-2" />
          稍后
        </Button>
        <Button
          :disabled="upgrading"
          @click="emit('restart')"
        >
          <RotateCcw class="h-4 w-4 mr-2" />
          {{ upgrading ? "重启中…" : "重启生效" }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>