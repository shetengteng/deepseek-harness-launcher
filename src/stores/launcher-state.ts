import { computed, ref, shallowRef } from "vue";
import type {
  BootstrapPlan,
  CrashLimitPayload,
  DshInstallProgressEvent,
  LatestDshVersion,
  LauncherErrorPayload,
  MirrorInfo,
} from "@/lib/tauri";
import type { LauncherPhase, LastAction, WizardStep } from "./launcher-types";

export function createLauncherState() {
  const phase = ref<LauncherPhase>("booting");
  const origin = ref<string | null>(null);
  const hostSession = ref(0);
  const dshVersion = ref<string | null>(null);
  const nodeVersion = ref<string | null>(null);
  const bootstrapPlan = ref<BootstrapPlan | null>(null);
  const latestDshVersion = ref<LatestDshVersion | null>(null);
  const error = shallowRef<LauncherErrorPayload | null>(null);
  const starting = ref(false);
  const stopping = ref(false);
  const installingDsh = ref(false);
  const dshInstallStage = ref<DshInstallProgressEvent["stage"]>("resolving");
  const dshInstallActivity = ref(0);
  const crashLimit = shallowRef<CrashLimitPayload | null>(null);
  const crashRecovering = ref(false);
  const autoRestartedAttempt = ref<number | null>(null);
  const lastFailedAction = ref<LastAction>(null);
  const preErrorPhase = ref<LauncherPhase | null>(null);
  const preErrorWizardStep = ref<WizardStep | null>(null);
  const wizardStep = ref<WizardStep>("mirror_select");
  const mirrors = ref<MirrorInfo[]>([]);
  const selectedMirrorId = ref<string | null>(null);
  const customMirrorUrl = ref("");
  const customMirrorValidation = ref<MirrorInfo | string | null>(null);
  const downloadProgress = ref({ bytes: 0, total: null as number | null });
  const downloadPercentHighWater = ref(0);
  const extractProgress = ref(0);
  const installing = ref(false);
  const nodeInstallOperationId = ref<string | null>(null);
  const dshInstallOperationId = ref<string | null>(null);
  const bootstrapping = ref(false);

  const selectedMirror = computed<MirrorInfo | null>(() => {
    if (selectedMirrorId.value === null) return null;
    if (
      customMirrorUrl.value &&
      typeof customMirrorValidation.value === "object" &&
      customMirrorValidation.value
    ) {
      return customMirrorValidation.value;
    }
    return (
      mirrors.value.find((mirror) => mirror.id === selectedMirrorId.value) ??
      null
    );
  });
  const downloadPercent = computed(() => {
    const { bytes, total } = downloadProgress.value;
    if (total === null || total === 0) return 0;
    const percent = Math.min(100, Math.round((bytes / total) * 100));
    if (percent < downloadPercentHighWater.value)
      return downloadPercentHighWater.value;
    downloadPercentHighWater.value = percent;
    return percent;
  });
  const dshInstallProgress = computed(() => {
    switch (dshInstallStage.value) {
      case "resolving":
        return 10;
      case "downloading":
        return Math.min(75, 20 + dshInstallActivity.value);
      case "installing":
        return 85;
      case "verifying":
        return 95;
    }
  });
  const displayPhase = computed<LauncherPhase>(() =>
    phase.value === "error" && preErrorPhase.value
      ? preErrorPhase.value === "booting"
        ? "idle"
        : preErrorPhase.value
      : phase.value,
  );
  const displayWizardStep = computed<WizardStep>(() =>
    phase.value === "error" && preErrorWizardStep.value
      ? preErrorWizardStep.value
      : wizardStep.value,
  );

  return {
    phase,
    origin,
    hostSession,
    dshVersion,
    nodeVersion,
    bootstrapPlan,
    latestDshVersion,
    error,
    starting,
    stopping,
    installingDsh,
    dshInstallStage,
    dshInstallActivity,
    crashLimit,
    crashRecovering,
    autoRestartedAttempt,
    lastFailedAction,
    preErrorPhase,
    preErrorWizardStep,
    wizardStep,
    mirrors,
    selectedMirrorId,
    customMirrorUrl,
    customMirrorValidation,
    downloadProgress,
    downloadPercentHighWater,
    extractProgress,
    installing,
    nodeInstallOperationId,
    dshInstallOperationId,
    bootstrapping,
    selectedMirror,
    downloadPercent,
    dshInstallProgress,
    displayPhase,
    displayWizardStep,
  };
}

export type LauncherState = ReturnType<typeof createLauncherState>;
