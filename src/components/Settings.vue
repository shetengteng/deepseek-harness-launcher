<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { RefreshCw } from "lucide-vue-next";
import SettingsEnvironmentCard from "@/components/settings/SettingsEnvironmentCard.vue";
import SettingsSourcesCard from "@/components/settings/SettingsSourcesCard.vue";
import SettingsSupportCard from "@/components/settings/SettingsSupportCard.vue";
import SettingsUpdateCard from "@/components/settings/SettingsUpdateCard.vue";
import {
  exportDiagnostics,
  getDshState,
  getLatestDshVersion,
  installDsh,
  listMirrors,
  restartHostAfterDshUpdate,
  setNodeMirror,
  setRegistry,
  uninstallManagedRuntime,
  type DshStateSnapshot,
  type LatestDshVersion,
  type MirrorInfo,
} from "@/lib/tauri";

const emit = defineEmits<{ upgradeReady: [origin: string] }>();
const props = withDefaults(
  defineProps<{
    nodeVersion?: string | null;
    exportDiagnosticsRequest?: number;
  }>(),
  {
    nodeVersion: null,
    exportDiagnosticsRequest: 0,
  },
);

const dshState = ref<DshStateSnapshot | null>(null);
const latestDshVersion = ref<LatestDshVersion | null>(null);
const nodeMirrors = ref<MirrorInfo[]>([]);
const nodeMirrorDraft = ref("");
const registryDraft = ref("");
const loading = ref(true);
const versionsLoading = ref(false);
const upgrading = ref(false);
const upgradeError = ref<string | null>(null);
const sourceError = ref<string | null>(null);
const exporting = ref(false);
const exportInfo = ref<string | null>(null);
const confirmingUninstall = ref(false);
const uninstalling = ref(false);
const uninstallError = ref<string | null>(null);

const messageOf = (error: unknown) =>
  typeof error === "object" && error !== null && "message" in error
    ? String((error as { message: unknown }).message)
    : String(error);

async function loadDshState(): Promise<void> {
  loading.value = true;
  try {
    const [state, mirrors, latest] = await Promise.all([
      getDshState(),
      listMirrors(),
      getLatestDshVersion(),
    ]);
    dshState.value = state;
    latestDshVersion.value = latest;
    nodeMirrors.value = mirrors;
    nodeMirrorDraft.value = state.node_mirror;
    registryDraft.value = state.registry;
  } catch {
    dshState.value = null;
  } finally {
    loading.value = false;
  }
}

async function refreshLatestDshVersion(): Promise<void> {
  versionsLoading.value = true;
  upgradeError.value = null;
  try {
    latestDshVersion.value = await getLatestDshVersion();
  } catch (error) {
    upgradeError.value = messageOf(error);
  } finally {
    versionsLoading.value = false;
  }
}

async function installLatestDsh(): Promise<void> {
  if (upgrading.value) return;
  upgrading.value = true;
  upgradeError.value = null;
  try {
    await installDsh();
    const restart = await restartHostAfterDshUpdate();
    await loadDshState();
    if (restart.rolled_back) {
      upgradeError.value = `新版本无法启动，已恢复 ${restart.active_version}。`;
    }
    emit("upgradeReady", restart.origin);
  } catch (error) {
    upgradeError.value = messageOf(error);
  } finally {
    upgrading.value = false;
  }
}

async function handleExportDiagnostics(): Promise<void> {
  exporting.value = true;
  exportInfo.value = null;
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const destination = await save({
      title: "导出诊断信息",
      defaultPath: `deepseek-harness-launcher-diagnostics-${new Date().toISOString().slice(0, 10)}.zip`,
      filters: [{ name: "ZIP 压缩包", extensions: ["zip"] }],
    });
    if (!destination) return;
    const size = await exportDiagnostics(destination);
    exportInfo.value = `已导出（${(size / 1024).toFixed(1)} KB）：${destination}`;
  } catch (error) {
    exportInfo.value = messageOf(error);
  } finally {
    exporting.value = false;
  }
}

async function handleUninstallManagedRuntime(): Promise<void> {
  uninstalling.value = true;
  uninstallError.value = null;
  try {
    await uninstallManagedRuntime();
  } catch (error) {
    uninstallError.value =
      typeof error === "object" && error !== null && "user_message" in error
        ? String(
            (error as { user_message?: unknown; message?: unknown })
              .user_message ??
              (error as { message?: unknown }).message ??
              error,
          )
        : String(error);
    uninstalling.value = false;
  }
}

async function handleSetNodeMirror(value: unknown): Promise<void> {
  if (typeof value !== "string" || !value) return;
  sourceError.value = null;
  nodeMirrorDraft.value = value;
  try {
    await setNodeMirror(value);
    if (dshState.value) dshState.value.node_mirror = value;
  } catch {
    sourceError.value = "未能保存 Node.js 下载来源，请重试。";
    nodeMirrorDraft.value = dshState.value?.node_mirror ?? "";
  }
}

async function handleSetRegistry(value: unknown): Promise<void> {
  if (typeof value !== "string" || !value) return;
  sourceError.value = null;
  registryDraft.value = value;
  try {
    await setRegistry(value);
    if (dshState.value) dshState.value.registry = value;
  } catch {
    sourceError.value = "未能保存 npm 下载源，请重试。";
    registryDraft.value = dshState.value?.registry ?? "";
  }
}

onMounted(loadDshState);
watch(
  () => props.exportDiagnosticsRequest,
  (request, previous) => {
    if (request > previous) void handleExportDiagnostics();
  },
);
</script>

<template>
  <div
    class="settings-panel flex min-h-0 flex-1 flex-col overflow-y-auto px-7 py-6"
  >
    <div class="space-y-4">
      <div
        v-if="loading"
        class="flex min-h-48 items-center justify-center text-muted-foreground"
        role="status"
      >
        <RefreshCw aria-hidden="true" class="h-5 w-5 animate-spin" />
        <span class="sr-only">正在加载设置</span>
      </div>
      <div v-else-if="!dshState" class="text-muted-foreground text-sm">
        无法加载设置
      </div>
      <template v-else>
        <SettingsEnvironmentCard
          :dsh-state="dshState"
          :node-version="props.nodeVersion"
        />
        <SettingsUpdateCard
          :dsh-state="dshState"
          :latest-version="latestDshVersion"
          :refreshing="versionsLoading"
          :upgrading="upgrading"
          :error="upgradeError"
          @refresh="refreshLatestDshVersion"
          @install="installLatestDsh"
        />
        <SettingsSourcesCard
          :node-mirrors="nodeMirrors"
          :node-mirror="nodeMirrorDraft"
          :registry="registryDraft"
          :error="sourceError"
          @set-node-mirror="handleSetNodeMirror"
          @set-registry="handleSetRegistry"
        />
        <SettingsSupportCard
          :exporting="exporting"
          :export-info="exportInfo"
          :confirming-uninstall="confirmingUninstall"
          :uninstalling="uninstalling"
          :uninstall-error="uninstallError"
          @export="handleExportDiagnostics"
          @confirm-uninstall="confirmingUninstall = true"
          @cancel-uninstall="confirmingUninstall = false"
          @uninstall="handleUninstallManagedRuntime"
        />
      </template>
    </div>
  </div>
</template>

<style scoped>
.settings-panel :deep(.rounded-lg.border.bg-card) {
  border: 0;
  border-bottom: 1px solid hsl(var(--border));
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}
.settings-panel :deep(.rounded-lg.border.bg-card > :first-child) {
  padding: 0 0 0.75rem;
}
.settings-panel :deep(.rounded-lg.border.bg-card > :not(:first-child)) {
  padding: 0.75rem 0 1.25rem;
}
</style>
