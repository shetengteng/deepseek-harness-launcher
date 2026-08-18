<script setup lang="ts">
import {
  CheckCircle2,
  ChevronDown,
  Loader2,
  Package,
  RotateCcw,
} from "lucide-vue-next";
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
import LauncherIcon from "@/components/LauncherIcon.vue";
import ThemeToggleButton from "@/components/theme/ThemeToggleButton.vue";
import LanguageToggleButton from "@/components/LanguageToggleButton.vue";
import { useFirstRunWizard } from "@/composables/useFirstRunWizard";
import { useI18n } from "@/lib/i18n";
import { useThemeStore } from "@/stores/theme";

const {
  store,
  nodeDone,
  nodeActive,
  nodeMessage,
  dshDone,
  dshMessage,
  nodeStatus,
  dshStatus,
  restartingDownload,
  canRestartDownload,
  restartingDshInstall,
  npmRegistry,
  restartNodeDownload,
  restartDshInstall,
} = useFirstRunWizard();
const theme = useThemeStore();
const { t } = useI18n();
</script>
<template>
  <main class="min-h-screen flex items-center justify-center bg-muted/50 p-6">
    <section
      class="relative w-[460px] max-w-full overflow-hidden rounded-lg bg-card shadow-2xl"
    >
      <div class="absolute right-3 top-3 flex items-start gap-1">
        <LanguageToggleButton />
        <ThemeToggleButton
        :mode="theme.mode"
        :disabled="theme.initializing || theme.saving"
        :saving="theme.saving"
        :error="theme.error"
        @change="theme.updateTheme"
        />
      </div>
      <div class="flex flex-col gap-[18px] p-[30px]">
        <div class="flex size-16 shrink-0 items-center justify-center">
          <LauncherIcon class="size-16" />
        </div>
        <div>
          <h1 class="text-lg font-semibold tracking-tight">{{ t("firstRun.preparing") }}</h1>
          <p class="mt-1 text-[13px] leading-5 text-muted-foreground">
            {{ t("firstRun.description") }}
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
                  ? t("firstRun.completed")
                  : store.downloadPercent
                    ? `${store.downloadPercent}%`
                    : t("firstRun.preparingStatus")
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
                  ? t("firstRun.completed")
                  : store.installingDsh
                    ? `${store.dshInstallProgress}%`
                    : t("firstRun.waiting")
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
            {{ nodeDone ? t("firstRun.switchNpm") : t("firstRun.switchDownload") }}
            <ChevronDown class="h-4 w-4" />
          </summary>
          <div class="mt-3 space-y-4 border-t pt-4">
            <template v-if="nodeDone">
              <p class="text-xs leading-5 text-muted-foreground">
                {{ t("firstRun.npmExplanation") }}
              </p>
              <div class="space-y-2">
                <Label for="dsh-registry">{{ t("firstRun.npmRegistry") }}</Label>
                <Select v-model="npmRegistry">
                  <SelectTrigger id="dsh-registry" class="w-full">
                    <SelectValue :placeholder="t('firstRun.selectSource')" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="https://registry.npmmirror.com">
                      npmmirror.com
                    </SelectItem>
                    <SelectItem value="https://registry.npmjs.org">
                      {{ t("firstRun.npmOfficial") }}
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
                    ? t("firstRun.restarting")
                    : t("firstRun.restartNpm")
                }}
              </Button>
            </template>
            <template v-else>
              <p class="text-xs leading-5 text-muted-foreground">
                {{ t("firstRun.nodeExplanation") }}
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
                  restartingDownload ? t("firstRun.restarting") : t("firstRun.restartSource")
                }}
              </Button>
            </template>
          </div>
        </details>
      </div>
    </section>
  </main>
</template>
