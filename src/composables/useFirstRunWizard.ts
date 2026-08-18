import { computed, onMounted, onUnmounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { useLauncherStore } from "@/stores/launcher";
import type { DshInstallProgressEvent, ProgressEvent } from "@/lib/tauri";
import { useI18n } from "@/lib/i18n";

export function useFirstRunWizard() {
  const store = useLauncherStore();
  const { t } = useI18n();
  let unlistenDownload: (() => void) | null = null;
  let unlistenExtract: (() => void) | null = null;
  let unlistenDsh: (() => void) | null = null;

  const nodeDone = computed(() => store.nodeVersion !== null);
  const nodeActive = computed(
    () => !nodeDone.value && store.wizardStep !== "resolving",
  );
  const nodeMessage = computed(() =>
    nodeDone.value
      ? t("firstRun.nodeVerified")
      : store.wizardStep === "resolving"
        ? t("firstRun.nodeResolving")
        : store.wizardStep === "extracting"
          ? t("firstRun.nodeExtracting")
          : t("firstRun.nodeDownloading"),
  );
  const dshDone = computed(() => store.dshVersion !== null);
  const dshMessage = computed(() =>
    dshDone.value
      ? t("firstRun.dshVerified")
      : store.installingDsh
        ? {
            resolving: t("firstRun.dshResolving"),
            downloading: t("firstRun.dshDownloading", {
              count: store.dshInstallActivity,
            }),
            installing: t("firstRun.dshInstalling"),
            verifying: t("firstRun.dshVerifying"),
          }[store.dshInstallStage]
        : nodeDone.value
          ? t("firstRun.dshNext")
          : t("firstRun.dshWaiting"),
  );
  const nodeStatus = computed(() =>
    nodeDone.value ? t("firstRun.statusComplete") : nodeMessage.value,
  );
  const dshStatus = computed(() =>
    dshDone.value ? t("firstRun.statusComplete") : dshMessage.value,
  );
  const restartingDownload = ref(false);
  const canRestartDownload = computed(
    () => store.installing && store.nodeInstallOperationId !== null,
  );
  const restartingDshInstall = ref(false);
  const npmRegistry = computed({
    get: () => store.bootstrapPlan?.registry ?? "https://registry.npmjs.org",
    set: (registry: string) => {
      if (store.bootstrapPlan) store.bootstrapPlan.registry = registry;
    },
  });

  async function restartNodeDownload(): Promise<void> {
    if (!canRestartDownload.value) return;
    restartingDownload.value = true;
    try {
      await store.restartNodeDownload();
    } finally {
      restartingDownload.value = false;
    }
  }

  async function restartDshInstall(): Promise<void> {
    restartingDshInstall.value = true;
    try {
      await store.restartDshInstall(npmRegistry.value);
    } finally {
      restartingDshInstall.value = false;
    }
  }

  onMounted(async () => {
    try {
      unlistenDownload = await listen<ProgressEvent>(
        "download-progress",
        (event) => store.applyProgressEvent(event.payload),
      );
      unlistenExtract = await listen<ProgressEvent>(
        "extract-progress",
        (event) => store.applyProgressEvent(event.payload),
      );
      unlistenDsh = await listen<DshInstallProgressEvent>(
        "dsh-install-progress",
        (event) => store.applyDshInstallProgress(event.payload),
      );
    } catch (error) {
      console.warn("Tauri event listen failed:", error);
    }
    void store.startBootstrap();
  });

  onUnmounted(() => {
    unlistenDownload?.();
    unlistenExtract?.();
    unlistenDsh?.();
  });

  return {
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
  };
}
