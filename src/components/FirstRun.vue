<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  CheckCircle2,
  ChevronDown,
  Loader2,
  Package,
  RotateCcw,
  Terminal,
} from "lucide-vue-next";
import { listen } from "@tauri-apps/api/event";
import { Progress } from "@/components/ui/progress";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import MirrorSelector from "@/components/MirrorSelector.vue";
import { useLauncherStore } from "@/stores/launcher";
import type { DshInstallProgressEvent, ProgressEvent } from "@/lib/tauri";

const store = useLauncherStore();
let unlistenDownload: (() => void) | null = null;
let unlistenExtract: (() => void) | null = null;
let unlistenDsh: (() => void) | null = null;
const nodeDone = computed(() => store.nodeVersion !== null);
const nodeActive = computed(
  () => !nodeDone.value && store.wizardStep !== "resolving",
);
const nodeMessage = computed(() =>
  nodeDone.value
    ? "SHA-256 已校验"
    : store.wizardStep === "resolving"
      ? "正在确认 DeepSeek Harness 版本与 Node.js 要求…"
      : store.wizardStep === "extracting"
        ? "正在解压并原子切换…"
        : "正在下载并校验运行时…",
);
const dshDone = computed(() => store.dshVersion !== null);
const dshMessage = computed(() =>
  dshDone.value
    ? "完整性已校验"
    : store.installingDsh
      ? {
          resolving: "正在准备依赖…",
          downloading: `npm install 进行中，已处理 ${store.dshInstallActivity} 个包…`,
          installing: "正在运行安装脚本…",
          verifying: "正在校验安装结果…",
        }[store.dshInstallStage]
      : nodeDone.value
        ? "即将自动安装…"
        : "等待 Node.js 运行时…",
);
const nodeStatus = computed(() =>
  nodeDone.value ? "✓ 已完成" : nodeMessage.value,
);
const dshStatus = computed(() =>
  dshDone.value ? "✓ 已完成" : dshMessage.value,
);
const restartingDownload = ref(false);
const canRestartDownload = computed(
  () => store.installing && store.nodeInstallOperationId !== null,
);
const restartingDshInstall = ref(false);
const npmRegistry = computed({
  get: () => store.bootstrapPlan?.registry ?? "https://registry.npmjs.org",
  set: (registry: string) => {
    if (store.bootstrapPlan) store.bootstrapPlan.registry = registry;
  },
});

async function restartNodeDownload(): Promise<void> {
  if (!canRestartDownload.value) return;
  restartingDownload.value = true;
  try {
    await store.restartNodeDownload();
  } finally {
    restartingDownload.value = false;
  }
}

async function restartDshInstall(): Promise<void> {
  restartingDshInstall.value = true;
  try {
    await store.restartDshInstall(npmRegistry.value);
  } finally {
    restartingDshInstall.value = false;
  }
}

onMounted(async () => {
  try {
    unlistenDownload = await listen<ProgressEvent>(
      "download-progress",
      (event) => store.applyProgressEvent(event.payload),
    );
    unlistenExtract = await listen<ProgressEvent>("extract-progress", (event) =>
      store.applyProgressEvent(event.payload),
    );
    unlistenDsh = await listen<DshInstallProgressEvent>(
      "dsh-install-progress",
      (event) => store.applyDshInstallProgress(event.payload),
    );
  } catch (error) {
    console.warn("Tauri event listen failed:", error);
  }
  void store.startBootstrap();
});
onUnmounted(() => {
  unlistenDownload?.();
  unlistenExtract?.();
  unlistenDsh?.();
});
</script>
<template>
  <main class="min-h-screen flex items-center justify-center bg-muted/50 p-6">
    <section
      class="w-[460px] max-w-full overflow-hidden rounded-lg bg-card shadow-2xl"
    >
      <div class="flex flex-col gap-[18px] p-[30px]">
        <div
          class="flex h-12 w-12 shrink-0 items-center justify-center rounded-md bg-primary text-primary-foreground"
        >
          <Terminal class="h-6 w-6" />
        </div>
        <div>
          <h1 class="text-lg font-semibold tracking-tight">正在准备运行环境</h1>
          <p class="mt-1 text-[13px] leading-5 text-muted-foreground">
            首次启动需要下载 Node.js 运行时和 DeepSeek Harness。
          </p>
        </div>

        <article class="rounded-lg border bg-card p-[14px]">
          <div class="flex items-center justify-between gap-3">
            <div
              class="flex min-w-0 items-center gap-2 text-[13px] font-medium"
            >
              <CheckCircle2
                v-if="nodeDone"
                class="h-4 w-4 shrink-0 text-success"
              />
              <Loader2
                v-else
                class="h-4 w-4 shrink-0 animate-spin text-muted-foreground"
              />
              <span class="truncate"
                >Node.js v{{ store.bootstrapPlan?.node_version ?? "…" }}</span
              >
            </div>
            <span class="shrink-0 font-mono text-[11px] text-muted-foreground">
              {{
                nodeDone
                  ? "已完成"
                  : store.downloadPercent
                    ? `${store.downloadPercent}%`
                    : "准备中"
              }}
            </span>
          </div>
          <Progress
            :class="
              nodeDone ? 'mt-[10px] h-2 [&>div]:bg-success' : 'mt-[10px] h-2'
            "
            :model-value="
              nodeDone ? 100 : nodeActive ? store.downloadPercent : 0
            "
          />
          <div
            class="mt-2 flex items-center justify-between gap-3 font-mono text-[11px] text-muted-foreground"
          >
            <span :class="nodeDone ? 'text-success' : ''">{{
              nodeStatus
            }}</span>
            <span v-if="nodeDone">{{ nodeMessage }}</span>
          </div>
        </article>

        <article class="rounded-lg border bg-card p-[14px]">
          <div class="flex items-center justify-between gap-3">
            <div
              class="flex min-w-0 items-center gap-2 text-[13px] font-medium"
            >
              <CheckCircle2
                v-if="dshDone"
                class="h-4 w-4 shrink-0 text-success"
              />
              <Loader2
                v-else-if="store.installingDsh"
                class="h-4 w-4 shrink-0 animate-spin text-muted-foreground"
              />
              <Package v-else class="h-4 w-4 shrink-0 text-muted-foreground" />
              <span class="truncate"
                >DeepSeek Harness
                {{
                  store.bootstrapPlan?.dsh_version ??
                  store.latestDshVersion?.latest_version ??
                  "…"
                }}</span
              >
            </div>
            <span class="shrink-0 font-mono text-[11px] text-muted-foreground">
              {{
                dshDone
                  ? "已完成"
                  : store.installingDsh
                    ? `${store.dshInstallProgress}%`
                    : "等待中"
              }}
            </span>
          </div>
          <Progress
            :class="
              dshDone ? 'mt-[10px] h-2 [&>div]:bg-success' : 'mt-[10px] h-2'
            "
            :model-value="
              dshDone ? 100 : store.installingDsh ? store.dshInstallProgress : 0
            "
          />
          <div
            class="mt-2 flex items-center justify-between gap-3 font-mono text-[11px] text-muted-foreground"
          >
            <span :class="dshDone ? 'text-success' : ''">{{ dshStatus }}</span>
            <span v-if="dshDone">{{ dshMessage }}</span>
          </div>
        </article>

        <details v-if="!dshDone" class="text-sm">
          <summary
            class="flex cursor-pointer list-none items-center gap-1 py-1 font-medium text-muted-foreground [&::-webkit-details-marker]:hidden"
          >
            {{ nodeDone ? "切换 npm 下载源" : "切换下载来源" }}
            <ChevronDown class="h-4 w-4" />
          </summary>
          <div class="mt-3 space-y-4 border-t pt-4">
            <template v-if="nodeDone">
              <p class="text-xs leading-5 text-muted-foreground">
                安装缓慢时可切换 npm
                下载源。重新开始会停止当前安装，并使用所选来源重新下载 DeepSeek
                Harness。
              </p>
              <div class="space-y-2">
                <Label for="dsh-registry">npm 下载源</Label>
                <Select v-model="npmRegistry">
                  <SelectTrigger id="dsh-registry" class="w-full">
                    <SelectValue placeholder="选择下载源" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="https://registry.npmmirror.com">
                      npmmirror.com
                    </SelectItem>
                    <SelectItem value="https://registry.npmjs.org">
                      npmjs.com（官方）
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <Button
                class="w-full"
                variant="outline"
                @click="restartDshInstall"
              >
                <RotateCcw
                  :class="[
                    'mr-2 h-4 w-4',
                    restartingDshInstall && 'animate-spin',
                  ]"
                />
                {{
                  restartingDshInstall
                    ? "正在重新开始…"
                    : "重新使用此 npm 来源下载"
                }}
              </Button>
            </template>
            <template v-else>
              <p class="text-xs leading-5 text-muted-foreground">
                下载缓慢时可切换 Node.js
                来源。重新开始会停止当前下载，并使用所选来源从头下载。
              </p>
              <MirrorSelector />
              <Button
                class="w-full"
                variant="outline"
                :disabled="!canRestartDownload || restartingDownload"
                @click="restartNodeDownload"
              >
                <RotateCcw
                  :class="[
                    'mr-2 h-4 w-4',
                    restartingDownload && 'animate-spin',
                  ]"
                />
                {{
                  restartingDownload ? "正在重新开始…" : "重新使用此来源下载"
                }}
              </Button>
            </template>
          </div>
        </details>
      </div>
    </section>
  </main>
</template>
