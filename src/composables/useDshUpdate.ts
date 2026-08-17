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
  checkDshUpdate,
  installDsh,
  restartHostAfterDshUpdate,
  type DshInstallProgressEvent,
} from "@/lib/tauri";
import { useLauncherStore } from "@/stores/launcher";

export type DshUpdateDialogState =
  "idle" | "installing" | "cancelling" | "restarting" | "failed";

export function useDshUpdate() {
  const store = useLauncherStore();
  const dshUpdateChecked = ref(false);
  const updateNotice = ref<ReturnType<typeof toast> | null>(null);
  const updateDialogState = ref<DshUpdateDialogState>("idle");
  const updateOperationId = ref<string | null>(null);
  const updateCurrentVersion = ref<string | null>(null);
  const updateTargetVersion = ref<string | null>(null);
  const updateError = ref<string | null>(null);
  const updateStage = ref<DshInstallProgressEvent["stage"]>("resolving");
  let unlistenUpdateProgress: (() => void) | null = null;

  const updateDialogOpen = computed(() => updateDialogState.value !== "idle");
  const updateInProgress = computed(
    () =>
      updateDialogState.value === "installing" ||
      updateDialogState.value === "cancelling" ||
      updateDialogState.value === "restarting",
  );
  const updateProgress = computed(() => {
    if (updateDialogState.value === "restarting") return 100;
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
  }

  function isDshInstallCancelled(error: unknown): boolean {
    return messageOf(error).includes("dsh installation was cancelled");
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
    } catch (error) {
      if (
        updateDialogState.value === "cancelling" ||
        isDshInstallCancelled(error)
      ) {
        closeUpdateDialog();
        return;
      }
      updateDialogState.value = "failed";
      updateError.value = messageOf(error);
    } finally {
      updateOperationId.value = null;
    }
  }

  function startDshUpdate(): void {
    if (updateInProgress.value || !updateTargetVersion.value) return;
    updateError.value = null;
    updateStage.value = "resolving";
    updateDialogState.value = "installing";
    void installDisplayedUpdate();
  }

  async function cancelDshUpdate(): Promise<void> {
    if (updateDialogState.value !== "installing") return;
    const operationId = updateOperationId.value;
    if (!operationId) return;

    updateDialogState.value = "cancelling";
    try {
      if (!(await cancelDshInstall(operationId))) {
        updateDialogState.value = "installing";
      }
    } catch (error) {
      console.warn("failed to cancel dsh update:", error);
      updateDialogState.value = "installing";
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
              disabled: updateInProgress.value,
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
    startDshUpdate,
    cancelDshUpdate,
    closeUpdateDialog,
  };
}
