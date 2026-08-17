<script setup lang="ts">
import { computed, onMounted, onUnmounted } from "vue";
import { CheckCircle2, Loader2, Package, Terminal } from "lucide-vue-next";
import { listen } from "@tauri-apps/api/event";
import { Progress } from "@/components/ui/progress";
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
      ? "正在确认 dsh 版本与 Node.js 要求…"
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
  <main class="min-h-screen flex items-center justify-center bg-background p-6">
    <section
      class="flex h-[540px] w-[460px] max-h-[calc(100vh-3rem)] max-w-full flex-col overflow-hidden rounded-lg bg-card shadow-2xl"
    >
      <div class="flex flex-1 flex-col gap-[18px] overflow-y-auto p-[30px]">
        <div
          class="flex h-12 w-12 items-center justify-center rounded-xl bg-primary text-primary-foreground"
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
                >@deepseek-ai/dsh
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
      </div>
    </section>
  </main>
</template>
