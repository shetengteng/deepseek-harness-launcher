<script setup lang="ts">
// 首启向导。对应设计 §M2.5 / PR-011。
// 步骤：镜像源选择 → 下载 → 解压 → 完成 → 启动 Host
// 监听 Tauri 事件 `download-progress` / `extract-progress` 更新进度条。

import { computed, onMounted, onUnmounted, watch } from "vue";
import {
  CheckCircle2,
  Download,
  Loader2,
  Package,
} from "lucide-vue-next";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { Badge } from "@/components/ui/badge";
import MirrorSelector from "@/components/MirrorSelector.vue";
import {
  DEFAULT_NODE_VERSION,
  useLauncherStore,
} from "@/stores/launcher";
import type { ProgressEvent } from "@/lib/tauri";

const store = useLauncherStore();

// 挂载时加载镜像源列表（refreshStatus 由 MainView 调用）
onMounted(() => {
  void store.loadMirrors();
});

// 监听 Tauri 事件
let unlistenDownload: (() => void) | null = null;
let unlistenExtract: (() => void) | null = null;

onMounted(async () => {
  try {
    unlistenDownload = await listen<ProgressEvent>("download-progress", (e) => {
      store.applyProgressEvent(e.payload);
    });
    unlistenExtract = await listen<ProgressEvent>("extract-progress", (e) => {
      store.applyProgressEvent(e.payload);
    });
  } catch (e) {
    // 在非 Tauri 环境（vitest）下 listen 会失败，这里静默
    console.warn("Tauri event listen failed:", e);
  }
});

onUnmounted(() => {
  unlistenDownload?.();
  unlistenExtract?.();
});

// 当向导进入 done 状态时，自动触发 refreshStatus 切到 idle（在 installNode action 内已调）
// 之后 MainView 的 watch 会切到 idle 视图

// 是否可以开始安装
const canInstall = computed(() => {
  return (
    store.selectedMirror !== null &&
    !store.installing &&
    store.displayWizardStep === "mirror_select"
  );
});

// 当前步骤索引（1-based，用于展示）
const stepIndex = computed(() => {
  switch (store.displayWizardStep) {
    case "mirror_select":
    case "probing":
      return 1;
    case "downloading":
      return 2;
    case "extracting":
      return 3;
    case "done":
      return 4;
    case "failed":
      return 0; // 失败不显示步骤号
  }
});

// 下载进度百分比
const downloadPercentValue = computed(() => store.downloadPercent);

// 解压进度百分比（0/50/100）
const extractPercentValue = computed(() => {
  return Math.round(store.extractProgress * 100);
});

// 触发安装
function onInstall() {
  void store.installNode();
}

// 启动 Host（向导完成后）
function onStartHost() {
  void store.startHost();
}

// 安装 dsh（向导完成 Node 后，dsh 未装时显示此按钮）
function onInstallDsh() {
  void store.installDsh();
}

// 监听 phase 变化：如果 wizardStep === "done" 且 phase 切到 idle，等待用户点击"启动 Host"
// 不做自动启动，让用户确认
watch(
  () => store.phase,
  () => {
    // 占位，未来可加自动启动逻辑
  },
);
</script>

<template>
  <main class="min-h-screen flex items-center justify-center p-6 bg-background">
    <Card class="w-[640px] max-w-full">
      <CardHeader>
        <div class="flex items-center justify-between">
          <CardTitle class="flex items-center gap-2">
            <Package class="h-5 w-5" />
            首次启动向导
          </CardTitle>
          <Badge variant="outline">步骤 {{ stepIndex }} / 4</Badge>
        </div>
      </CardHeader>

      <CardContent class="flex flex-col gap-6">
        <!-- Step 1: 镜像源选择 -->
        <div
          v-if="store.displayWizardStep === 'mirror_select' || store.displayWizardStep === 'probing'"
          class="flex flex-col gap-4"
        >
          <div class="flex flex-col gap-1">
            <h3 class="text-base font-medium">选择 Node 下载源</h3>
            <p class="text-sm text-muted-foreground">
              将下载 Node.js v{{ DEFAULT_NODE_VERSION }}（约 30MB）
            </p>
          </div>
          <MirrorSelector />

          <div class="flex justify-end items-center pt-2">
            <Button :disabled="!canInstall" @click="onInstall">
              <Download class="h-4 w-4 mr-2" />
              开始下载
            </Button>
          </div>
        </div>

        <!-- Step 2: 下载中 -->
        <div
          v-else-if="store.displayWizardStep === 'downloading'"
          class="flex flex-col gap-4"
        >
          <div class="flex flex-col gap-1">
            <h3 class="text-base font-medium flex items-center gap-2">
              <Loader2 class="h-4 w-4 animate-spin" />
              正在下载 Node.js v{{ DEFAULT_NODE_VERSION }}
            </h3>
            <p class="text-sm text-muted-foreground">
              镜像源：{{ store.selectedMirror?.name ?? "未知" }}
            </p>
          </div>
          <div class="flex flex-col gap-2">
            <Progress :model-value="downloadPercentValue" />
            <div class="flex justify-between text-xs text-muted-foreground">
              <span>{{ downloadPercentValue }}%</span>
              <span>请稍候…</span>
            </div>
          </div>
        </div>

        <!-- Step 3: 解压中 -->
        <div
          v-else-if="store.displayWizardStep === 'extracting'"
          class="flex flex-col gap-4"
        >
          <div class="flex flex-col gap-1">
            <h3 class="text-base font-medium flex items-center gap-2">
              <Loader2 class="h-4 w-4 animate-spin" />
              正在解压并安装…
            </h3>
            <p class="text-sm text-muted-foreground">
              下载已完成，正在原子切换
            </p>
          </div>
          <Progress :model-value="extractPercentValue" />
        </div>

        <!-- Step 4: 完成 -->
        <div v-else-if="store.displayWizardStep === 'done'" class="flex flex-col gap-4">
          <div class="flex flex-col gap-2 items-center py-4">
            <CheckCircle2 class="h-12 w-12 text-green-500" />
            <h3 class="text-base font-medium">Node 安装完成</h3>
            <p class="text-sm text-muted-foreground">
              Node.js v{{ store.nodeVersion }} 已就绪
            </p>
          </div>

          <!-- dsh 未装：显示"安装 DeepSeek Harness"按钮 -->
          <div v-if="!store.dshVersion" class="flex flex-col gap-3">
            <Button
              :disabled="store.installingDsh"
              @click="onInstallDsh"
            >
              <Loader2
                v-if="store.installingDsh"
                class="h-4 w-4 mr-2 animate-spin"
              />
              <Download v-else class="h-4 w-4 mr-2" />
              {{ store.installingDsh ? "安装 DeepSeek Harness 中…" : "安装 DeepSeek Harness" }}
            </Button>

            <!-- 安装中：不确定进度条 + 状态提示 -->
            <div v-if="store.installingDsh" class="flex flex-col gap-2">
              <Progress :model-value="0" class="[&>div]:animate-pulse" />
              <p class="text-xs text-muted-foreground text-center">
                正在通过 npm 拉取依赖包，预计 10~30 秒，请耐心等待…
              </p>
            </div>
          </div>

          <!-- dsh 已装：显示"启动 DeepSeek Harness"按钮 -->
          <Button v-else :disabled="store.starting" @click="onStartHost">
            <Loader2 v-if="store.starting" class="h-4 w-4 mr-2 animate-spin" />
            {{ store.starting ? "启动中…" : "启动 DeepSeek Harness" }}
          </Button>
        </div>

        <!-- 失败分支已移除：错误统一由 ErrorDialog 处理（覆盖在原视图上） -->
      </CardContent>
    </Card>
  </main>
</template>
