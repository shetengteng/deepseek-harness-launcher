import {
  getLatestDshVersion,
  installDsh,
  installNode,
  listMirrors,
  probeMirrors,
  resolveBootstrapPlan,
  validateCustomMirror,
  type DshInstallProgressEvent,
  type LauncherErrorPayload,
  type ProgressEvent,
} from "@/lib/tauri";
import { detectPlatformArch } from "./launcher-platform";
import type { LauncherState } from "./launcher-state";
import type { LastAction } from "./launcher-types";

type FailureHandler = (error: unknown, action?: LastAction) => void;

type BootstrapDependencies = {
  state: LauncherState;
  fail: FailureHandler;
  refreshStatus: () => Promise<void>;
  startHost: () => Promise<void>;
};

export function createBootstrapActions({
  state,
  fail,
  refreshStatus,
  startHost,
}: BootstrapDependencies) {
  async function loadLatestDshVersionAction(): Promise<void> {
    try {
      state.latestDshVersion.value = await getLatestDshVersion();
    } catch (error) {
      fail(error, "bootstrap");
    }
  }

  async function loadMirrors(): Promise<void> {
    try {
      state.mirrors.value = await listMirrors();
      if (
        state.selectedMirrorId.value === null &&
        state.mirrors.value.length > 0
      ) {
        state.selectedMirrorId.value = state.mirrors.value[0]!.id;
      }
    } catch (error) {
      fail(error);
    }
  }

  async function autoPickMirror(): Promise<void> {
    const previousStep = state.wizardStep.value;
    state.wizardStep.value = "probing";
    try {
      const custom = state.customMirrorUrl.value
        ? [state.customMirrorUrl.value]
        : undefined;
      const picked = await probeMirrors(custom);
      state.selectedMirrorId.value = picked.id;
      if (!state.mirrors.value.some((mirror) => mirror.id === picked.id)) {
        state.mirrors.value = [...state.mirrors.value, picked];
      }
      state.wizardStep.value =
        previousStep === "probing" ? "mirror_select" : previousStep;
    } catch (error) {
      state.wizardStep.value = "mirror_select";
      state.error.value = toLauncherError(error);
      state.phase.value = "error";
    }
  }

  async function validateCustomMirrorAction(url: string): Promise<void> {
    if (!url) {
      state.customMirrorValidation.value = null;
      return;
    }
    try {
      state.customMirrorValidation.value = await validateCustomMirror(url);
    } catch (error) {
      state.customMirrorValidation.value = errorMessage(error);
    }
  }

  function selectMirror(id: string): void {
    state.selectedMirrorId.value = id;
  }

  async function installNodeAction(options?: {
    version?: string;
  }): Promise<void> {
    if (state.installing.value) return;
    state.installing.value = true;
    state.wizardStep.value = "resolving";
    resetProgress();
    try {
      const mirror = state.selectedMirror.value;
      if (!mirror) throw new Error("未选择镜像源");
      const plan = state.bootstrapPlan.value ?? (await resolveBootstrapPlan());
      state.bootstrapPlan.value = plan;
      state.wizardStep.value = "downloading";
      const { platform, arch } = detectPlatformArch();
      await installNode({
        version: options?.version ?? plan.node_version,
        mirrorBaseUrl: mirror.base_url,
        platform,
        arch,
      });
      state.wizardStep.value = "done";
      await refreshStatus();
    } catch (error) {
      resetProgress();
      fail(error, "installNode");
    } finally {
      state.installing.value = false;
    }
  }

  async function installDshAction(): Promise<void> {
    if (state.installingDsh.value) return;
    state.installingDsh.value = true;
    state.dshInstallStage.value = "resolving";
    state.dshInstallActivity.value = 0;
    try {
      state.bootstrapPlan.value ??= await resolveBootstrapPlan();
      await installDsh();
      state.dshInstallStage.value = "verifying";
      await refreshStatus();
      state.lastFailedAction.value = null;
    } catch (error) {
      fail(error, "installDsh");
    } finally {
      state.installingDsh.value = false;
    }
  }

  async function startBootstrapAction(): Promise<void> {
    if (state.bootstrapping.value || state.phase.value === "ready") return;
    state.bootstrapping.value = true;
    try {
      if (!state.nodeVersion.value) {
        state.wizardStep.value = "resolving";
        state.bootstrapPlan.value ??= await resolveBootstrapPlan();
        await loadMirrors();
        if (state.error.value) {
          state.lastFailedAction.value = "bootstrap";
          return;
        }
        await autoPickMirror();
        if (state.error.value) {
          state.lastFailedAction.value = "bootstrap";
          return;
        }
        await installNodeAction();
        if (state.error.value) return;
      }
      if (!state.dshVersion.value) {
        await installDshAction();
        if (state.error.value) return;
      }
      if (state.nodeVersion.value && state.dshVersion.value) await startHost();
    } catch (error) {
      fail(error, "bootstrap");
    } finally {
      state.bootstrapping.value = false;
    }
  }

  function applyProgressEvent(event: ProgressEvent): void {
    if (event.stage === "download") {
      state.downloadProgress.value = { bytes: event.bytes, total: event.total };
    } else if (event.stage === "extract") {
      if (event.total === 0) state.extractProgress.value = 1;
      else {
        state.wizardStep.value = "extracting";
        state.extractProgress.value = 0.5;
      }
    }
  }

  function applyDshInstallProgress(event: DshInstallProgressEvent): void {
    state.dshInstallStage.value = event.stage;
    if (event.stage === "downloading") state.dshInstallActivity.value += 1;
  }

  function resetWizard(): void {
    state.wizardStep.value = "mirror_select";
    resetProgress();
    state.error.value = null;
  }

  function resetProgress(): void {
    state.downloadProgress.value = { bytes: 0, total: null };
    state.downloadPercentHighWater.value = 0;
    state.extractProgress.value = 0;
  }

  return {
    loadLatestDshVersionAction,
    loadMirrors,
    autoPickMirror,
    validateCustomMirrorAction,
    selectMirror,
    installNodeAction,
    installDshAction,
    startBootstrapAction,
    applyProgressEvent,
    applyDshInstallProgress,
    resetWizard,
  };
}

function errorMessage(error: unknown): string {
  return typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string"
    ? (error as { message: string }).message
    : error instanceof Error
      ? error.message
      : String(error);
}

function toLauncherError(error: unknown): LauncherErrorPayload {
  if (
    typeof error === "object" &&
    error !== null &&
    "kind" in error &&
    "message" in error
  ) {
    return error as LauncherErrorPayload;
  }
  return { kind: "io", message: errorMessage(error) };
}
