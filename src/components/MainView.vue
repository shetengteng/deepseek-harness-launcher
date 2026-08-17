<script setup lang="ts">
// 主视图。对应设计 §M1.5 + §M2.5（PR-011）+ §M3.5（PR-016）+ §5.5（PR-017 崩溃恢复）。
// 按 phase 渲染：booting → Loading；first_run → FirstRun 向导；idle → 启动/安装按钮；ready → iframe。设置以弹框覆盖当前视图。

import {
  computed,
  defineComponent,
  h,
  nextTick,
  onMounted,
  ref,
  watch,
} from "vue";
import { Download, Play, RefreshCw } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import CrashDialog from "@/components/CrashDialog.vue";
import ErrorDialog from "@/components/ErrorDialog.vue";
import FirstRun from "@/components/FirstRun.vue";
import HostStarting from "@/components/HostStarting.vue";
import SettingsView from "@/components/Settings.vue";
import AboutDialog from "@/components/AboutDialog.vue";
import { ToastAction, toast } from "@/components/ui/toast";
import { useTrayEvents } from "@/composables/useTrayEvents";
import {
  checkDshUpdate,
  installDsh,
  restartHostAfterDshUpdate,
} from "@/lib/tauri";
import { useLauncherStore } from "@/stores/launcher";

const store = useLauncherStore();

const showSettings = ref(false);
const showAbout = ref(false);
const exportDiagnosticsRequest = ref(0);
const dshUpdateChecked = ref(false);
const updatingFromNotice = ref(false);
const updateNotice = ref<ReturnType<typeof toast> | null>(null);

function handleUpgradeReady(origin: string): void {
  store.setHostReady(origin);
}

useTrayEvents({
  openSettings: () => {
    showSettings.value = true;
  },
  checkDshUpdate: () => {
    showSettings.value = true;
  },
  exportDiagnostics: () => {
    showSettings.value = true;
    void nextTick(() => {
      exportDiagnosticsRequest.value += 1;
    });
  },
  openAbout: () => {
    showAbout.value = true;
  },
  hostRestarted: () => {
    void store.startHost();
  },
});

onMounted(async () => {
  await store.refreshStatus();
  if (store.phase === "idle" && store.nodeVersion && store.dshVersion) {
    void store.startHost();
  }
  void store.initCrashEvents();
});

/** 是否需要先装 dsh 才能启动 Host。 */
const needInstallDsh = computed(() => store.dshVersion === null);

function messageOf(error: unknown): string {
  if (typeof error === "object" && error !== null) {
    if ("user_message" in error && error.user_message)
      return String(error.user_message);
    if ("message" in error) return String(error.message);
  }
  return String(error);
}

async function installLatestFromNotice(expectedVersion: string): Promise<void> {
  if (updatingFromNotice.value) return;

  updatingFromNotice.value = true;
  updateNotice.value?.update({
    title: "正在更新",
    description: "正在安装新版 DeepSeek Harness，完成后会自动重启服务。",
  });

  try {
    const version = await installDsh({ expectedVersion });
    const restart = await restartHostAfterDshUpdate();
    store.dshVersion = restart.active_version;
    store.setHostReady(restart.origin);
    if (restart.rolled_back) {
      updateNotice.value?.update({
        title: "更新未完成",
        description: `版本 ${version} 无法启动，已恢复 ${restart.active_version}。`,
      });
      return;
    }
    updateNotice.value?.dismiss();
  } catch (error) {
    updateNotice.value?.update({
      title: "更新失败",
      description: messageOf(error),
    });
  } finally {
    updatingFromNotice.value = false;
  }
}

function createUpdateToastAction(expectedVersion: string) {
  return defineComponent({
    name: "UpdateToastAction",
    setup() {
      return () =>
        h(
          ToastAction,
          {
            altText: "立即更新 DeepSeek Harness",
            disabled: updatingFromNotice.value,
            onClick: () => void installLatestFromNotice(expectedVersion),
          },
          () => [
            h(RefreshCw, {
              class: ["size-3.5", updatingFromNotice.value && "animate-spin"],
            }),
            updatingFromNotice.value ? "更新中…" : "立即更新",
          ],
        );
    },
  });
}

async function checkDshUpdateAfterStart(): Promise<void> {
  if (dshUpdateChecked.value) return;
  dshUpdateChecked.value = true;
  try {
    const update = await checkDshUpdate();
    if (!update) return;

    updateNotice.value = toast({
      type: "background",
      duration: Number.POSITIVE_INFINITY,
      title: "发现新版本",
      description: `当前 ${update.current_version}，可更新至 ${update.latest_version}。更新会在你确认后开始，并自动重启服务。`,
      action: createUpdateToastAction(update.latest_version),
    });
  } catch {
    // 更新检查失败不能影响当前 dsh 会话。
  }
}

watch(
  () => store.displayPhase,
  (phase) => {
    if (phase === "ready") void checkDshUpdateAfterStart();
  },
  { immediate: true },
);
</script>

<template>
  <main class="min-h-screen flex flex-col">
    <!-- 启动中：初始状态查询与 dsh Host 就绪前均使用原型 04 遮罩。 -->
    <HostStarting
      v-if="
        store.displayPhase === 'booting' ||
        store.starting ||
        store.crashRecovering
      "
    />

    <!-- 首启向导（原型 03） -->
    <FirstRun v-else-if="store.displayPhase === 'first_run'" />

    <!-- idle：根据 dsh/node 状态显示不同按钮 -->
    <div
      v-else-if="store.displayPhase === 'idle'"
      class="flex-1 flex items-center justify-center p-6"
    >
      <Card class="w-[480px]">
        <CardHeader>
          <CardTitle>DeepSeek Harness</CardTitle>
        </CardHeader>
        <CardContent class="flex flex-col gap-3 text-sm">
          <div class="flex justify-between">
            <span class="text-muted-foreground">DeepSeek Harness 版本</span>
            <span>{{ store.dshVersion ?? "未安装" }}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-muted-foreground">Node 版本</span>
            <span>{{ store.nodeVersion ?? "未托管" }}</span>
          </div>

          <!-- dsh 未装：显示"安装 DeepSeek Harness"按钮 -->
          <Button
            v-if="needInstallDsh"
            :disabled="store.installingDsh"
            @click="store.installDsh()"
          >
            <Download class="h-4 w-4 mr-2" />
            {{ store.installingDsh ? "安装中…" : "安装 DeepSeek Harness" }}
          </Button>

          <!-- dsh 已装：显示"启动 DeepSeek Harness"按钮 -->
          <Button v-else :disabled="store.starting" @click="store.startHost()">
            <Play class="h-4 w-4 mr-2" />
            {{ store.starting ? "启动中…" : "启动 DeepSeek Harness" }}
          </Button>
        </CardContent>
      </Card>
    </div>

    <!-- ready：iframe 加载 dsh web -->
    <template v-else-if="store.displayPhase === 'ready' && store.origin">
      <div
        v-if="store.autoRestartedAttempt !== null"
        class="flex items-center px-3 py-1 border-b bg-background"
      >
        <span class="text-xs text-muted-foreground">
          DeepSeek Harness 曾意外退出，已自动恢复（第
          {{ store.autoRestartedAttempt }} 次）
        </span>
      </div>
      <iframe
        :src="store.origin"
        class="flex-1 w-full border-0"
        sandbox="allow-same-origin allow-scripts allow-forms allow-popups allow-modals"
      />
    </template>

    <!-- 错误对话框：覆盖在原视图上，根据 lastFailedAction 决定重试行为 -->
    <ErrorDialog
      :error="store.error"
      :last-failed-action="store.lastFailedAction"
      @dismiss="store.resetError()"
      @retry="store.retryLastAction()"
    />

    <!-- 崩溃恢复弹窗（PR-017）：达到重试上限或自动重启失败时弹出 -->
    <CrashDialog
      :crash="store.crashLimit"
      :recovering="store.crashRecovering"
      @retry="store.retryAfterCrash()"
      @rollback="store.rollbackAfterCrash()"
      @dismiss="store.dismissCrash()"
    />

    <Dialog :open="showSettings" @update:open="showSettings = $event">
      <DialogContent
        v-if="showSettings"
        class="h-[640px] w-[640px] max-h-[calc(100vh-2rem)] max-w-[calc(100vw-2rem)] gap-0 overflow-hidden p-0"
      >
        <DialogHeader
          class="h-10 shrink-0 justify-center gap-0 border-b px-4 text-center"
        >
          <DialogTitle class="text-sm font-medium">
            deepseek-harness-launcher 设置
          </DialogTitle>
          <DialogDescription class="sr-only">
            管理 DeepSeek Harness 的运行时与升级策略。
          </DialogDescription>
        </DialogHeader>
        <SettingsView
          class="min-h-0 flex-1"
          :node-version="store.nodeVersion"
          :host-origin="store.origin"
          :export-diagnostics-request="exportDiagnosticsRequest"
          @upgrade-ready="handleUpgradeReady"
        />
      </DialogContent>
    </Dialog>

    <AboutDialog
      :open="showAbout"
      :host-origin="store.origin"
      @close="showAbout = false"
    />

  </main>
</template>
