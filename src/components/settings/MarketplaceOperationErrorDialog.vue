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
import type { MarketplaceOperation } from "@/lib/tauri";

interface Props {
  error: string | null;
  operation: MarketplaceOperation | null;
  canRetry: boolean;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  dismiss: [];
  retry: [];
}>();

function operationTitle(operation: MarketplaceOperation | null): string {
  return operation?.kind === "remove" ? "插件卸载失败" : "插件安装失败";
}
</script>

<template>
  <Dialog
    :open="props.error !== null"
    @update:open="(open) => !open && emit('dismiss')"
  >
    <DialogContent class="sm:max-w-[520px]">
      <DialogHeader>
        <DialogTitle>{{ operationTitle(props.operation) }}</DialogTitle>
        <DialogDescription
          v-if="props.error"
          class="whitespace-pre-wrap break-words"
        >
          {{ props.error }}
        </DialogDescription>
      </DialogHeader>

      <div
        v-if="props.operation?.log_path"
        class="rounded border bg-muted/30 p-3 text-xs"
      >
        <div class="mb-1 text-muted-foreground">操作日志</div>
        <code class="block break-all font-mono">{{
          props.operation.log_path
        }}</code>
      </div>

      <DialogFooter>
        <Button variant="outline" @click="emit('dismiss')">关闭</Button>
        <Button v-if="props.canRetry" @click="emit('retry')">重试</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
