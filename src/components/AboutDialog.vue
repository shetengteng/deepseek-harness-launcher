<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  Dialog,
  DialogContent,
  DialogTitle,
} from "@/components/ui/dialog";
import { getAboutInfo, type AboutInfo } from "@/lib/tauri";
import LauncherIcon from "@/components/LauncherIcon.vue";

const props = withDefaults(
  defineProps<{
    open: boolean;
    hostOrigin?: string | null;
  }>(),
  { hostOrigin: null },
);

const emit = defineEmits<{
  (e: "close"): void;
}>();

const aboutInfo = ref<AboutInfo | null>(null);
const error = ref<string | null>(null);

const endpoint = computed(() => {
  if (!props.hostOrigin) return "未运行";
  try {
    return new URL(props.hostOrigin).host;
  } catch {
    return props.hostOrigin;
  }
});

async function loadAboutInfo(): Promise<void> {
  error.value = null;
  try {
    aboutInfo.value = await getAboutInfo();
  } catch {
    aboutInfo.value = null;
    error.value = "无法读取运行时信息。";
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) void loadAboutInfo();
  },
  { immediate: true },
);
</script>

<template>
  <Dialog :open="open" @update:open="(value) => !value && emit('close')">
    <DialogContent
      class="h-[640px] w-[640px] max-h-[calc(100vh-2rem)] max-w-[calc(100vw-2rem)] content-start gap-6 overflow-y-auto"
    >
      <section class="space-y-2 text-center">
        <div class="mx-auto flex size-16 items-center justify-center">
          <LauncherIcon class="size-16" />
        </div>
        <DialogTitle class="text-base">deepseek-harness-launcher</DialogTitle>
        <p class="font-mono text-xs text-muted-foreground">
          v{{ aboutInfo?.launcher_version ?? "…" }}
        </p>
      </section>

      <p v-if="error" class="text-sm text-destructive">{{ error }}</p>
      <dl
        v-else
        class="overflow-hidden rounded-md border text-sm divide-y divide-border"
      >
        <div class="grid grid-cols-[11rem_minmax(0,1fr)] gap-4 px-3 py-2.5">
          <dt class="text-muted-foreground">启动器版本</dt>
          <dd class="font-mono">{{ aboutInfo?.launcher_version ?? "读取中…" }}</dd>
        </div>
        <div class="grid grid-cols-[11rem_minmax(0,1fr)] gap-4 px-3 py-2.5">
          <dt class="text-muted-foreground">DeepSeek Harness 版本</dt>
          <dd class="font-mono">{{ aboutInfo?.dsh_version ?? "尚未安装" }}</dd>
        </div>
        <div class="grid grid-cols-[11rem_minmax(0,1fr)] gap-4 px-3 py-2.5">
          <dt class="text-muted-foreground">Node.js 版本</dt>
          <dd class="font-mono">{{ aboutInfo?.node_version ?? "尚未安装" }}</dd>
        </div>
        <div class="grid grid-cols-[11rem_minmax(0,1fr)] gap-4 px-3 py-2.5">
          <dt class="text-muted-foreground">数据目录</dt>
          <dd class="min-w-0 break-all font-mono text-xs">
            {{ aboutInfo?.data_directory ?? "读取中…" }}
          </dd>
        </div>
        <div class="grid grid-cols-[11rem_minmax(0,1fr)] gap-4 px-3 py-2.5">
          <dt class="text-muted-foreground">DeepSeek Harness 端点</dt>
          <dd class="font-mono">{{ endpoint }}</dd>
        </div>
      </dl>
    </DialogContent>
  </Dialog>
</template>
