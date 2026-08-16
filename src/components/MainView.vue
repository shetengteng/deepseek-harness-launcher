<script setup lang="ts">
// 主视图。对应设计 §M1.5 + §M2.5（PR-011）+ §M3.5（PR-016）。
// 按 phase 渲染：booting → Loading；first_run → FirstRun 向导；idle → 启动/安装按钮；ready → iframe；settings → 设置页。

import { computed, onMounted, ref } from "vue";
import { Loader2, Download, Play, Settings } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import ErrorDialog from "@/components/ErrorDialog.vue";
import FirstRun from "@/components/FirstRun.vue";
import SettingsView from "@/components/Settings.vue";
import { useLauncherStore } from "@/stores/launcher";

const store = useLauncherStore();

const showSettings = ref(false);

onMounted(() => {
  void store.refreshStatus();
});

/** 是否需要先装 dsh 才能启动 Host。 */
const needInstallDsh = computed(() => store.dshVersion === null);
</script>

<template>
  <main class="min-h-screen flex flex-col">
    <!-- 设置页 -->
    <SettingsView
      v-if="showSettings"
      @back="showSettings = false"
    />

    <template v-else>
      <!-- 启动中 -->
      <div
        v-if="store.displayPhase === 'booting'"
        class="flex-1 flex items-center justify-center"
      >
        <div class="flex flex-col items-center gap-3 text-muted-foreground">
          <Loader2 class="h-8 w-8 animate-spin" />
          <span>正在初始化…</span>
        </div>
      </div>

      <!-- 首启向导（PR-011） -->
      <FirstRun v-else-if="store.displayPhase === 'first_run'" />

      <!-- idle：根据 dsh/node 状态显示不同按钮 -->
      <div
        v-else-if="store.displayPhase === 'idle'"
        class="flex-1 flex items-center justify-center p-6"
      >
        <Card class="w-[480px]">
          <CardHeader>
            <div class="flex items-center justify-between">
              <CardTitle>DeepSeek Harness</CardTitle>
              <Button
                variant="ghost"
                size="icon"
                @click="showSettings = true"
              >
                <Settings class="h-5 w-5" />
              </Button>
            </div>
          </CardHeader>
          <CardContent class="flex flex-col gap-3 text-sm">
            <div class="flex justify-between">
              <span class="text-muted-foreground">Harness 版本</span>
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
        <!-- 顶部工具栏 -->
        <div class="flex items-center justify-end px-3 py-1 border-b bg-background">
          <Button
            variant="ghost"
            size="icon"
            @click="showSettings = true"
          >
            <Settings class="h-5 w-5" />
          </Button>
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
    </template>
  </main>
</template>
