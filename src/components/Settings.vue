<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { listen } from "@tauri-apps/api/event";
import LauncherIcon from "@/components/LauncherIcon.vue";
import SettingsCommandCard from "@/components/settings/SettingsCommandCard.vue";
import SettingsAppearanceCard from "@/components/settings/SettingsAppearanceCard.vue";
import SettingsEnvironmentCard from "@/components/settings/SettingsEnvironmentCard.vue";
import SettingsSourcesCard from "@/components/settings/SettingsSourcesCard.vue";
import SettingsSupportCard from "@/components/settings/SettingsSupportCard.vue";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { useThemeStore } from "@/stores/theme";
import { useI18n } from "@/lib/i18n";
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
const theme = useThemeStore();
const { t } = useI18n();
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
const nodeUpdateStage = ref<"downloading" | "extracting" | "complete">(
  "downloading",
);
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
  return total && total > 0
    ? Math.min(90, Math.round((bytes / total) * 90))
    : 8;
});

const nodeUpdateMessage = computed(() => {
  if (nodeUpdateStage.value === "complete") return t("settings.nodeComplete");
  if (nodeUpdateStage.value === "extracting")
    return t("settings.nodeExtracting");
  return t("settings.nodeDownloading");
});

const nodeUpdateActionLabel = computed(() =>
  manualNodeTarget.value?.update_available
    ? t("settings.nodeOnly")
    : t("settings.nodeReinstall"),
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
  if (preparingNodeUpdate.value || updatingNode.value || upgrading.value)
    return;
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

async function cancelManualNodeUpdate(): Promise<void> {
  if (updatingNode.value) {
    const operationId = nodeUpdateOperationId.value;
    if (!operationId) return;
    try {
      await cancelNodeInstall(operationId);
    } catch (error) {
      manualNodeUpdateError.value = messageOf(error);
    }
    return;
  }
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
    upgradeError.value = t("settings.rollback", { version: restart.active_version });
  }
  emit("upgradeReady", restart.origin);
}

async function handleExportDiagnostics(): Promise<void> {
  exporting.value = true;
  exportInfo.value = null;
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const destination = await save({
      title: t("settings.exportTitle"),
      defaultPath: `deepseek-harness-launcher-diagnostics-${new Date().toISOString().slice(0, 10)}.zip`,
      filters: [{ name: t("settings.zip"), extensions: ["zip"] }],
    });
    if (!destination) return;
    const size = await exportDiagnostics(destination);
    exportInfo.value = t("settings.exported", {
      size: (size / 1024).toFixed(1),
      destination,
    });
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
    sourceError.value = t("settings.nodeSourceSaveFailed");
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
    sourceError.value = t("settings.npmSourceSaveFailed");
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

function handleThemeChange(mode: "light" | "dark"): void {
  void theme.updateTheme(mode);
}

let unlistenDownloadProgress: (() => void) | null = null;
let unlistenExtractProgress: (() => void) | null = null;

onMounted(() => {
  void loadDshState();
  void (async () => {
    try {
      unlistenDownloadProgress = await listen<ProgressEvent>(
        "download-progress",
        (event) => {
          if (!updatingNode.value || event.payload.stage !== "download") return;
          nodeUpdateStage.value = "downloading";
          nodeDownloadProgress.value = {
            bytes: event.payload.bytes,
            total: event.payload.total,
          };
        },
      );
      unlistenExtractProgress = await listen<ProgressEvent>(
        "extract-progress",
        () => {
          if (updatingNode.value) nodeUpdateStage.value = "extracting";
        },
      );
    } catch (error) {
      console.warn("Tauri event listen failed:", error);
    }
  })();
});

onUnmounted(() => {
  unlistenDownloadProgress?.();
  unlistenExtractProgress?.();
});

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
        <LauncherIcon
          aria-hidden="true"
          class="size-16 shrink-0 animate-none"
        />
        <span
          aria-hidden="true"
          class="h-8 w-8 animate-spin rounded-full border-2 border-border border-t-primary"
        />
        <span class="sr-only">{{ t("settings.loading") }}</span>
      </section>
    </div>
    <div v-else class="space-y-4">
      <div v-if="!dshState" class="text-muted-foreground text-sm">
        {{ t("settings.loadFailed") }}
      </div>
      <template v-else>
        <SettingsAppearanceCard
          :mode="theme.mode"
          :disabled="theme.initializing || theme.saving"
          :error="theme.error"
          @change="handleThemeChange"
        />
        <SettingsEnvironmentCard
          :dsh-state="dshState"
          :node-version="props.nodeVersion"
          :host-origin="props.hostOrigin"
          :latest-version="latestDshVersion"
          :refreshing="versionsLoading"
          :upgrading="upgrading"
          :error="upgradeError"
          :node-update-loading="
            preparingNodeUpdate || updatingNode || upgrading
          "
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
          <DialogTitle>{{ t("update.nodeRequired") }}</DialogTitle>
          <DialogDescription v-if="nodeUpgrade">
            {{ t("settings.nodeUpgradeDescription", { dshVersion: nodeUpgrade.dsh_version, requiredVersion: nodeUpgrade.engines_node, currentVersion: nodeUpgrade.current_node, targetVersion: nodeUpgrade.suggested_node }) }}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2 sm:gap-2">
          <Button
            variant="outline"
            :disabled="upgrading"
            @click="cancelNodeUpgrade"
          >
            {{ t("update.cancel") }}
          </Button>
          <Button :disabled="upgrading" @click="confirmNodeUpgrade">
            {{ upgrading ? t("settings.upgrading") : t("update.confirm") }}
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
          <DialogTitle>{{ t("settings.nodeUpdateTitle") }}</DialogTitle>
          <DialogDescription v-if="manualNodeTarget">
            {{ t("settings.nodeUpdateDescription", { currentVersion: manualNodeTarget.current_version, targetVersion: manualNodeTarget.target_version }) }}
          </DialogDescription>
        </DialogHeader>
        <div
          v-if="manualNodeTarget"
          class="rounded-md border bg-muted/30 px-3 py-2 text-sm"
        >
          <template v-if="manualNodeTarget.engines_node">
            <span class="text-muted-foreground">{{ t("settings.compatibility") }}</span>
            <span class="font-mono text-xs">{{
              manualNodeTarget.engines_node
            }}</span>
          </template>
          <span v-else class="text-xs text-muted-foreground">
            {{ t("settings.noCompatibility") }}
          </span>
        </div>
        <div v-if="updatingNode" class="space-y-2">
          <Progress :model-value="nodeUpdateProgress" class="h-2" />
          <div
            class="flex justify-between text-xs text-muted-foreground"
            role="status"
          >
            <span>{{ nodeUpdateMessage }}</span>
            <span>{{ nodeUpdateProgress }}%</span>
          </div>
        </div>
        <p
          v-if="manualNodeUpdateError"
          class="text-sm text-destructive"
          role="alert"
        >
          {{ manualNodeUpdateError }}
        </p>
        <DialogFooter class="gap-2 sm:gap-2">
          <Button variant="outline" @click="cancelManualNodeUpdate">
            {{ updatingNode ? t("settings.cancelDownload") : t("common.cancel") }}
          </Button>
          <Button :disabled="updatingNode" @click="confirmManualNodeUpdate">
            {{ updatingNode ? t("settings.updating") : nodeUpdateActionLabel }}
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
