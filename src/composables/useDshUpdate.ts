import {
  computed,
  defineComponent,
  h,
  onMounted,
  onUnmounted,
  ref,
  watch,
} from "vue";
import { listen } from "@tauri-apps/api/event";
import { RefreshCw } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import {
  cancelDshInstall,
  cancelNodeInstall,
  checkDshUpdate,
  installDsh,
  parseNodeUpgradeRequired,
  restartHostAfterDshUpdate,
  upgradeNode,
  type DshInstallProgressEvent,
  type NodeUpgradeRequired,
} from "@/lib/tauri";
import { useLauncherStore } from "@/stores/launcher";

export type DshUpdateDialogState =
  | "idle"
  | "confirming_node"
  | "upgrading_node"
  | "installing"
  | "cancelling"
  | "restarting"
  | "failed";

export function useDshUpdate() {
  const store = useLauncherStore();
  const dshUpdateChecked = ref(false);
  const updateNotice = ref<ReturnType<typeof toast> | null>(null);
  const updateDialogState = ref<DshUpdateDialogState>("idle");
  const updateOperationId = ref<string | null>(null);
  const updateCurrentVersion = ref<string | null>(null);
  const updateTargetVersion = ref<string | null>(null);
  const updateError = ref<string | null>(null);
  const nodeUpgrade = ref<NodeUpgradeRequired | null>(null);
  const updateStage = ref<DshInstallProgressEvent["stage"]>("resolving");
  let unlistenUpdateProgress: (() => void) | null = null;

  const updateDialogOpen = computed(() => updateDialogState.value !== "idle");
  const updateInProgress = computed(
    () =>
      updateDialogState.value === "upgrading_node" ||
      updateDialogState.value === "installing" ||
      updateDialogState.value === "cancelling" ||
      updateDialogState.value === "restarting",
  );
  const updateBusy = computed(
    () => updateInProgress.value || updateDialogState.value === "confirming_node",
  );
  const updateProgress = computed(() => {
    if (updateDialogState.value === "restarting") return 100;
    if (updateDialogState.value === "upgrading_node") return 28;
    switch (updateStage.value) {
      case "resolving":
        return 12;
      case "downloading":
        return 45;
      case "installing":
        return 72;
      case "verifying":
        return 90;
    }
  });
  const updateStageMessage = computed(() => {
    if (updateDialogState.value === "cancelling") return "正在取消安装…";
    if (updateDialogState.value === "restarting") return "正在重启 dsh…";
    if (updateDialogState.value === "upgrading_node") {
      return `正在下载并切换 Node ${nodeUpgrade.value?.suggested_node ?? ""}…`;
    }
    return {
      resolving: "正在从当前下载源获取最新版本…",
      downloading: "正在下载依赖…",
      installing: "正在安装依赖…",
      verifying: "正在校验安装结果…",
    }[updateStage.value];
  });

  function messageOf(error: unknown): string {
    if (typeof error === "object" && error !== null) {
      if ("user_message" in error && error.user_message)
        return String(error.user_message);
      if ("message" in error) return String(error.message);
    }
    return String(error);
  }

  function closeUpdateDialog(): void {
    updateDialogState.value = "idle";
    updateOperationId.value = null;
    updateError.value = null;
    nodeUpgrade.value = null;
  }

  function isCancelled(error: unknown): boolean {
    if (typeof error === "object" && error !== null && "kind" in error) {
      if (String((error as { kind: unknown }).kind) === "node_install_cancelled") {
        return true;
      }
    }
    const message = messageOf(error);
    return (
      message.includes("dsh installation was cancelled") ||
      message.includes("node installation cancelled") ||
      message.includes("已取消 Node")
    );
  }

  async function finishInstalledUpdate(version: string): Promise<void> {
    updateDialogState.value = "restarting";
    const restart = await restartHostAfterDshUpdate();
    store.dshVersion = restart.active_version;
    store.setHostReady(restart.origin);
    if (restart.rolled_back) {
      updateDialogState.value = "failed";
      updateError.value = `版本 ${version} 无法启动，已恢复 ${restart.active_version}。`;
      return;
    }
    updateNotice.value?.dismiss();
    updateNotice.value = null;
    closeUpdateDialog();
  }

  async function installDisplayedUpdate(): Promise<void> {
    const expectedVersion = updateTargetVersion.value;
    if (!expectedVersion) {
      updateDialogState.value = "failed";
      updateError.value = "未获取到可安装的新版本，请重新检查更新。";
      return;
    }
    const operationId = crypto.randomUUID();
    updateOperationId.value = operationId;
    try {
      const version = await installDsh({ operationId, expectedVersion });
      await finishInstalledUpdate(version);
    } catch (error) {
      if (updateDialogState.value === "cancelling" || isCancelled(error)) {
        closeUpdateDialog();
        return;
      }
      const required = parseNodeUpgradeRequired(error);
      if (required) {
        nodeUpgrade.value = required;
        updateDialogState.value = "confirming_node";
        return;
      }
      updateDialogState.value = "failed";
      updateError.value = messageOf(error);
    } finally {
      updateOperationId.value = null;
    }
  }

  function startDshUpdate(): void {
    if (updateBusy.value || !updateTargetVersion.value) return;
    updateError.value = null;
    nodeUpgrade.value = null;
    updateStage.value = "resolving";
    updateDialogState.value = "installing";
    void installDisplayedUpdate();
  }

  async function confirmNodeUpgrade(): Promise<void> {
    const required = nodeUpgrade.value;
    const expectedVersion = updateTargetVersion.value;
    if (!required || !expectedVersion || updateInProgress.value) return;

    const operationId = crypto.randomUUID();
    updateOperationId.value = operationId;
    updateDialogState.value = "upgrading_node";
    try {
      store.nodeVersion = await upgradeNode({
        version: required.suggested_node,
        operationId,
      });
      updateStage.value = "resolving";
      updateDialogState.value = "installing";
      const dshOperationId = crypto.randomUUID();
      updateOperationId.value = dshOperationId;
      const version = await installDsh({
        operationId: dshOperationId,
        expectedVersion,
      });
      await finishInstalledUpdate(version);
    } catch (error) {
      if (isCancelled(error) || String(updateDialogState.value) === "cancelling") {
        closeUpdateDialog();
        return;
      }
      updateDialogState.value = "failed";
      updateError.value = messageOf(error);
    } finally {
      updateOperationId.value = null;
    }
  }

  async function cancelDshUpdate(): Promise<void> {
    if (updateDialogState.value === "confirming_node") {
      closeUpdateDialog();
      return;
    }
    if (
      updateDialogState.value !== "installing" &&
      updateDialogState.value !== "upgrading_node"
    ) {
      return;
    }
    const operationId = updateOperationId.value;
    if (!operationId) return;

    const previous = updateDialogState.value;
    updateDialogState.value = "cancelling";
    try {
      const cancelled =
        previous === "upgrading_node"
          ? await cancelNodeInstall(operationId)
          : await cancelDshInstall(operationId);
      if (!cancelled) {
        updateDialogState.value = previous;
      }
    } catch (error) {
      console.warn("failed to cancel dsh update:", error);
      updateDialogState.value = previous;
    }
  }

  function createUpdateToastAction() {
    return defineComponent({
      name: "UpdateToastAction",
      setup() {
        return () =>
          h(
            Button,
            {
              size: "xs",
              disabled: updateBusy.value,
              onClick: startDshUpdate,
            },
            () => [
              h(RefreshCw, {
                class: ["size-3.5", updateInProgress.value && "animate-spin"],
              }),
              updateInProgress.value ? "更新中…" : "立即更新",
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

      updateCurrentVersion.value = update.current_version;
      updateTargetVersion.value = update.latest_version;
      updateNotice.value = toast({
        type: "background",
        duration: Number.POSITIVE_INFINITY,
        title: "发现新版本",
        description: `当前 ${update.current_version}，可更新至 ${update.latest_version}。更新会在你确认后开始，并自动重启服务。`,
        action: createUpdateToastAction(),
      });
    } catch {
      // 更新检查失败不能影响当前 dsh 会话。
    }
  }

  onMounted(async () => {
    try {
      unlistenUpdateProgress = await listen<DshInstallProgressEvent>(
        "dsh-install-progress",
        (event) => {
          if (updateDialogState.value === "installing") {
            updateStage.value = event.payload.stage;
          }
        },
      );
    } catch (error) {
      console.warn("Tauri event listen failed:", error);
    }
  });

  onUnmounted(() => {
    unlistenUpdateProgress?.();
  });

  watch(
    () => store.displayPhase,
    (phase) => {
      if (phase === "ready") void checkDshUpdateAfterStart();
    },
    { immediate: true },
  );

  return {
    updateDialogOpen,
    updateDialogState,
    updateInProgress,
    updateProgress,
    updateStageMessage,
    updateCurrentVersion,
    updateTargetVersion,
    updateError,
    nodeUpgrade,
    startDshUpdate,
    confirmNodeUpgrade,
    cancelDshUpdate,
    closeUpdateDialog,
  };
}
