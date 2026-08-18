<script setup lang="ts">
// 主视图。对应设计 §M1.5 + §M2.5（PR-011）+ §M3.5（PR-016）+ §5.5（PR-017 崩溃恢复）。
// 按 phase 渲染：booting → Loading；first_run → FirstRun 向导；idle → 启动/安装按钮；ready → iframe。设置作为独立页面替换当前视图。

import { computed, nextTick, onMounted, ref } from "vue";
import { Download, Play } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import CrashDialog from "@/components/CrashDialog.vue";
import DshUpdateDialog from "@/components/DshUpdateDialog.vue";
import ErrorDialog from "@/components/ErrorDialog.vue";
import FirstRun from "@/components/FirstRun.vue";
import HostStarting from "@/components/HostStarting.vue";
import SettingsPage from "@/components/SettingsPage.vue";
import AboutDialog from "@/components/AboutDialog.vue";
import { useDshExternalLinks } from "@/composables/useDshExternalLinks";
import { useTrayEvents } from "@/composables/useTrayEvents";
import { useLauncherStore } from "@/stores/launcher";

type SettingsSection = "settings" | "plugins";

const store = useLauncherStore();
const { dshFrame } = useDshExternalLinks();

const showSettingsPage = ref(false);
const settingsSection = ref<SettingsSection>("settings");
const showAbout = ref(false);
const exportDiagnosticsRequest = ref(0);

const needInstallDsh = computed(() => store.dshVersion === null);

function handleUpgradeReady(origin: string): void {
  store.setHostReady(origin);
}

function handleNodeUpdated(version: string): void {
  store.nodeVersion = version;
}

function openSettingsPage(section: SettingsSection): void {
  settingsSection.value = section;
  showSettingsPage.value = true;
}

useTrayEvents({
  openSettings: () => {
    openSettingsPage("settings");
  },
  openPlugins: () => {
    openSettingsPage("plugins");
  },
  checkDshUpdate: () => {
    openSettingsPage("settings");
  },
  exportDiagnostics: () => {
    openSettingsPage("settings");
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
</script>

<template>
  <main class="min-h-screen flex flex-col">
    <SettingsPage
      v-if="showSettingsPage"
      :node-version="store.nodeVersion"
      :host-origin="store.origin"
      :export-diagnostics-request="exportDiagnosticsRequest"
      :section="settingsSection"
      @close="showSettingsPage = false"
      @upgrade-ready="handleUpgradeReady"
      @node-updated="handleNodeUpdated"
    />

    <template v-else>
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
            <Button
              v-else
              :disabled="store.starting"
              @click="store.startHost()"
            >
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
          ref="dshFrame"
          :src="store.origin"
          class="flex-1 w-full border-0"
          allow="
            camera;
            microphone;
            geolocation;
            display-capture;
            clipboard-read;
            clipboard-write;
          "
          sandbox="allow-same-origin allow-scripts allow-forms allow-popups allow-modals"
        />
      </template>
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

    <DshUpdateDialog @open-settings="openSettingsPage('settings')" />

    <AboutDialog
      :open="showAbout"
      :host-origin="store.origin"
      @close="showAbout = false"
    />
  </main>
</template>
