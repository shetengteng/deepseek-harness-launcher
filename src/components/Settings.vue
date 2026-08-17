<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import LauncherIcon from "@/components/LauncherIcon.vue";
import SettingsCommandCard from "@/components/settings/SettingsCommandCard.vue";
import SettingsEnvironmentCard from "@/components/settings/SettingsEnvironmentCard.vue";
import SettingsSourcesCard from "@/components/settings/SettingsSourcesCard.vue";
import SettingsSupportCard from "@/components/settings/SettingsSupportCard.vue";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  cancelNodeInstall,
  exportDiagnostics,
  getDshState,
  getLatestDshVersion,
  getNodeUpdateTarget,
  installDshCli,
  installDsh,
  listMirrors,
  parseNodeUpgradeRequired,
  restartHostAfterDshUpdate,
  setNodeMirror,
  setRegistry,
  uninstallManagedRuntime,
  upgradeNode,
  type DshStateSnapshot,
  type DshCliInstallResult,
  type LatestDshVersion,
  type MirrorInfo,
  type NodeUpdateTarget,
  type NodeUpgradeRequired,
  type ProgressEvent,
} from "@/lib/tauri";

const emit = defineEmits<{
  upgradeReady: [origin: string];
  nodeUpdated: [version: string];
}>();
const props = withDefaults(
  defineProps<{
    nodeVersion?: string | null;
    hostOrigin?: string | null;
    exportDiagnosticsRequest?: number;
  }>(),
  {
    nodeVersion: null,
    hostOrigin: null,
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
const nodeUpgrade = ref<NodeUpgradeRequired | null>(null);
const manualNodeTarget = ref<NodeUpdateTarget | null>(null);
const preparingNodeUpdate = ref(false);
const updatingNode = ref(false);
const manualNodeUpdateError = ref<string | null>(null);
const nodeUpdateStage = ref<"downloading" | "extracting" | "complete">("downloading");
const nodeDownloadProgress = ref<{ bytes: number; total: number | null }>({
  bytes: 0,
  total: null,
});
const nodeUpdateOperationId = ref<string | null>(null);
const installingDshCli = ref(false);
const dshCliInstall = ref<DshCliInstallResult | null>(null);
const dshCliError = ref<string | null>(null);

const messageOf = (error: unknown) =>
  typeof error === "object" && error !== null && "message" in error
    ? String((error as { message: unknown }).message)
    : String(error);

const nodeUpdateProgress = computed(() => {
  if (nodeUpdateStage.value === "complete") return 100;
  if (nodeUpdateStage.value === "extracting") return 92;
  const { bytes, total } = nodeDownloadProgress.value;
  return total && total > 0 ? Math.min(90, Math.round((bytes / total) * 90)) : 8;
});

const nodeUpdateMessage = computed(() => {
  if (nodeUpdateStage.value === "complete") return "已完成原子切换";
  if (nodeUpdateStage.value === "extracting") return "正在解压、校验并切换 Node.js…";
  return "正在下载并校验 Node.js 运行时…";
});

const nodeUpdateActionLabel = computed(() =>
  manualNodeTarget.value?.update_available ? "仅更新 Node" : "重新安装 Node",
);

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
  const expectedVersion = latestDshVersion.value?.latest_version;
  if (upgrading.value || !expectedVersion) return;
  upgrading.value = true;
  upgradeError.value = null;
  try {
    await installDsh({ expectedVersion });
    await finishLatestInstall();
  } catch (error) {
    const required = parseNodeUpgradeRequired(error);
    if (required) {
      nodeUpgrade.value = required;
      return;
    }
    upgradeError.value = messageOf(error);
  } finally {
    upgrading.value = false;
  }
}

async function confirmNodeUpgrade(): Promise<void> {
  const required = nodeUpgrade.value;
  const expectedVersion = latestDshVersion.value?.latest_version;
  if (upgrading.value || !required || !expectedVersion) return;
  upgrading.value = true;
  upgradeError.value = null;
  try {
    await upgradeNode({
      version: required.suggested_node,
      operationId: crypto.randomUUID(),
    });
    nodeUpgrade.value = null;
    await installDsh({ expectedVersion });
    await finishLatestInstall();
  } catch (error) {
    upgradeError.value = messageOf(error);
  } finally {
    upgrading.value = false;
  }
}

function cancelNodeUpgrade(): void {
  nodeUpgrade.value = null;
}

async function prepareNodeUpdate(): Promise<void> {
  if (preparingNodeUpdate.value || updatingNode.value || upgrading.value) return;
  preparingNodeUpdate.value = true;
  manualNodeUpdateError.value = null;
  try {
    const target = await getNodeUpdateTarget();
    manualNodeTarget.value = target;
  } catch (error) {
    manualNodeUpdateError.value = messageOf(error);
  } finally {
    preparingNodeUpdate.value = false;
  }
}

function cancelManualNodeUpdate(): void {
  if (updatingNode.value) return;
  manualNodeTarget.value = null;
  manualNodeUpdateError.value = null;
}

async function confirmManualNodeUpdate(): Promise<void> {
  const target = manualNodeTarget.value;
  if (!target || updatingNode.value || upgrading.value) return;
  updatingNode.value = true;
  manualNodeUpdateError.value = null;
  nodeUpdateStage.value = "downloading";
  nodeDownloadProgress.value = { bytes: 0, total: null };
  const operationId = crypto.randomUUID();
  nodeUpdateOperationId.value = operationId;
  try {
    const version = await upgradeNode({
      version: target.target_version,
      operationId,
    });
    nodeUpdateStage.value = "complete";
    manualNodeTarget.value = null;
    emit("nodeUpdated", version);
  } catch (error) {
    manualNodeUpdateError.value = messageOf(error);
  } finally {
    nodeUpdateOperationId.value = null;
    updatingNode.value = false;
  }
}

async function finishLatestInstall(): Promise<void> {
  const restart = await restartHostAfterDshUpdate();
  await loadDshState();
  if (restart.rolled_back) {
    upgradeError.value = `新版本无法启动，已恢复 ${restart.active_version}。`;
  }
  emit("upgradeReady", restart.origin);
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

async function handleInstallDshCli(): Promise<void> {
  if (installingDshCli.value) return;
  installingDshCli.value = true;
  dshCliError.value = null;
  try {
    dshCliInstall.value = await installDshCli();
  } catch (error) {
    dshCliError.value = messageOf(error);
  } finally {
    installingDshCli.value = false;
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
    class="settings-panel relative flex min-h-0 flex-1 flex-col overflow-y-auto px-7 py-6"
  >
    <div
      v-if="loading"
      class="absolute inset-0 flex items-center justify-center"
      role="status"
    >
      <section class="flex flex-col items-center gap-[18px]">
        <LauncherIcon aria-hidden="true" class="size-16 shrink-0 animate-none" />
        <span
          aria-hidden="true"
          class="h-8 w-8 animate-spin rounded-full border-2 border-border border-t-primary"
        />
        <span class="sr-only">正在加载设置</span>
      </section>
    </div>
    <div v-else class="space-y-4">
      <div v-if="!dshState" class="text-muted-foreground text-sm">
        无法加载设置
      </div>
      <template v-else>
        <SettingsEnvironmentCard
          :dsh-state="dshState"
          :node-version="props.nodeVersion"
          :host-origin="props.hostOrigin"
          :latest-version="latestDshVersion"
          :refreshing="versionsLoading"
          :upgrading="upgrading"
          :error="upgradeError"
          :node-update-loading="preparingNodeUpdate || updatingNode || upgrading"
          :node-update-error="manualNodeUpdateError"
          @refresh="refreshLatestDshVersion"
          @install="installLatestDsh"
          @update-node="prepareNodeUpdate"
        />
        <SettingsCommandCard
          :installing="installingDshCli"
          :result="dshCliInstall"
          :error="dshCliError"
          @install="handleInstallDshCli"
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

    <Dialog :open="nodeUpgrade !== null">
      <DialogContent
        class="sm:max-w-[420px]"
        @escape-key-down.prevent
        @pointer-down-outside.prevent
      >
        <DialogHeader>
          <DialogTitle>需要升级 Node</DialogTitle>
          <DialogDescription v-if="nodeUpgrade">
            dsh {{ nodeUpgrade.dsh_version }} 需要 Node
            {{ nodeUpgrade.engines_node }}，当前为
            {{ nodeUpgrade.current_node }}。确认后将下载 Node
            {{ nodeUpgrade.suggested_node }} 并继续更新。
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2 sm:gap-2">
          <Button variant="outline" :disabled="upgrading" @click="cancelNodeUpgrade">
            取消更新
          </Button>
          <Button :disabled="upgrading" @click="confirmNodeUpgrade">
            {{ upgrading ? "升级中…" : "确认升级并继续" }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog :open="manualNodeTarget !== null">
      <DialogContent
        class="sm:max-w-[420px]"
        @escape-key-down.prevent
        @pointer-down-outside.prevent
      >
        <DialogHeader>
          <DialogTitle>更新 Node.js</DialogTitle>
          <DialogDescription v-if="manualNodeTarget">
            将从 {{ manualNodeTarget.current_version }} 更新至
            {{ manualNodeTarget.target_version }}。该操作只更新 Node.js，不安装或切换 dsh，
            运行中的 dsh 会继续使用当前进程。
          </DialogDescription>
        </DialogHeader>
        <div
          v-if="manualNodeTarget"
          class="rounded-md border bg-muted/30 px-3 py-2 text-sm"
        >
          <span class="text-muted-foreground">兼容要求：</span>
          <span class="font-mono text-xs">{{ manualNodeTarget.engines_node }}</span>
        </div>
        <DialogFooter class="gap-2 sm:gap-2">
          <Button variant="outline" :disabled="updatingNode" @click="cancelManualNodeUpdate">
            取消
          </Button>
          <Button :disabled="updatingNode" @click="confirmManualNodeUpdate">
            {{ updatingNode ? "更新中…" : "仅更新 Node" }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
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
