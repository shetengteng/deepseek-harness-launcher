import { computed, onMounted, onUnmounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { useLauncherStore } from "@/stores/launcher";
import type { DshInstallProgressEvent, ProgressEvent } from "@/lib/tauri";

export function useFirstRunWizard() {
  const store = useLauncherStore();
  let unlistenDownload: (() => void) | null = null;
  let unlistenExtract: (() => void) | null = null;
  let unlistenDsh: (() => void) | null = null;

  const nodeDone = computed(() => store.nodeVersion !== null);
  const nodeActive = computed(
    () => !nodeDone.value && store.wizardStep !== "resolving",
  );
  const nodeMessage = computed(() =>
    nodeDone.value
      ? "SHA-256 已校验"
      : store.wizardStep === "resolving"
        ? "正在确认 DeepSeek Harness 版本与 Node.js 要求…"
        : store.wizardStep === "extracting"
          ? "正在解压并原子切换…"
          : "正在下载并校验运行时…",
  );
  const dshDone = computed(() => store.dshVersion !== null);
  const dshMessage = computed(() =>
    dshDone.value
      ? "完整性已校验"
      : store.installingDsh
        ? {
            resolving: "正在准备依赖…",
            downloading: `npm install 进行中，已处理 ${store.dshInstallActivity} 个包…`,
            installing: "正在运行安装脚本…",
            verifying: "正在校验安装结果…",
          }[store.dshInstallStage]
        : nodeDone.value
          ? "即将自动安装…"
          : "等待 Node.js 运行时…",
  );
  const nodeStatus = computed(() =>
    nodeDone.value ? "✓ 已完成" : nodeMessage.value,
  );
  const dshStatus = computed(() =>
    dshDone.value ? "✓ 已完成" : dshMessage.value,
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
