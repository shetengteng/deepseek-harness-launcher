import { defineStore } from "pinia";
import {
  fetchStatus,
  type LauncherErrorPayload,
  type StatusSnapshot,
} from "@/lib/tauri";
import { createBootstrapActions } from "./launcher-bootstrap";
import { createHostActions } from "./launcher-host";
import { detectPlatformArch, setPlatformArch } from "./launcher-platform";
import { createLauncherState } from "./launcher-state";
import { DEFAULT_NODE_VERSION, type LastAction } from "./launcher-types";

export { DEFAULT_NODE_VERSION, detectPlatformArch };
export type { LauncherPhase, WizardStep } from "./launcher-types";

export const useLauncherStore = defineStore("launcher", () => {
  const state = createLauncherState();

  function fail(error: unknown, action: LastAction = null): void {
    if (action !== null) state.lastFailedAction.value = action;
    state.preErrorPhase.value = state.phase.value;
    state.preErrorWizardStep.value = state.wizardStep.value;
    state.error.value = toLauncherError(error);
    state.phase.value = "error";
  }

  function applySnapshot(snapshot: StatusSnapshot): void {
    setPlatformArch(snapshot.platform, snapshot.arch);
    state.dshVersion.value = snapshot.dsh_version;
    state.nodeVersion.value = snapshot.node_version;
    if (!(state.phase.value === "ready" && snapshot.host_origin === null)) {
      state.origin.value = snapshot.host_origin;
    }
    if (snapshot.phase === "ready" && snapshot.host_origin) {
      state.phase.value = "ready";
      return;
    }
    if (snapshot.phase === "first_run") {
      if (
        state.phase.value !== "first_run" ||
        state.wizardStep.value === "mirror_select"
      ) {
        state.wizardStep.value = snapshot.node_version
          ? "done"
          : "mirror_select";
      }
      state.phase.value = "first_run";
      return;
    }
    if (state.phase.value !== "ready") state.phase.value = "idle";
  }

  async function refreshStatus(): Promise<void> {
    try {
      applySnapshot(await fetchStatus());
    } catch (error) {
      fail(error);
    }
  }

  const host = createHostActions(state, fail, refreshStatus);
  const bootstrap = createBootstrapActions({
    state,
    fail,
    refreshStatus,
    startHost: host.startHostAction,
  });

  function resetError(): void {
    if (state.error.value === null) return;
    const kind = state.error.value.kind;
    state.error.value = null;
    if (kind === "node_not_installed" || kind === "dsh_not_installed") {
      state.phase.value = "first_run";
      state.wizardStep.value = "resolving";
      return;
    }
    if (state.preErrorPhase.value)
      state.phase.value = state.preErrorPhase.value;
    if (state.preErrorWizardStep.value)
      state.wizardStep.value = state.preErrorWizardStep.value;
  }

  async function retryLastAction(): Promise<void> {
    const action = state.lastFailedAction.value;
    state.error.value = null;
    if (state.preErrorPhase.value)
      state.phase.value = state.preErrorPhase.value;
    if (state.preErrorWizardStep.value)
      state.wizardStep.value = state.preErrorWizardStep.value;
    switch (action) {
      case "bootstrap":
        await bootstrap.startBootstrapAction();
        break;
      case "installNode":
        await bootstrap.installNodeAction();
        break;
      case "installDsh":
        await bootstrap.installDshAction();
        break;
      case "startHost":
        await host.startHostAction();
        break;
      case "shutdownHost":
        await host.shutdownHostAction();
        break;
      case null:
        break;
    }
  }

  return {
    ...state,
    refreshStatus,
    startHost: host.startHostAction,
    shutdownHost: host.shutdownHostAction,
    retryAfterCrash: host.retryAfterCrash,
    rollbackAfterCrash: host.rollbackAfterCrash,
    dismissCrash: host.dismissCrash,
    initCrashEvents: host.initCrashEvents,
    resetError,
    retryLastAction,
    startBootstrap: bootstrap.startBootstrapAction,
    loadLatestDshVersion: bootstrap.loadLatestDshVersionAction,
    loadMirrors: bootstrap.loadMirrors,
    autoPickMirror: bootstrap.autoPickMirror,
    validateCustomMirror: bootstrap.validateCustomMirrorAction,
    selectMirror: bootstrap.selectMirror,
    installNode: bootstrap.installNodeAction,
    restartNodeDownload: bootstrap.restartNodeDownloadAction,
    installDsh: bootstrap.installDshAction,
    applyProgressEvent: bootstrap.applyProgressEvent,
    applyDshInstallProgress: bootstrap.applyDshInstallProgress,
    resetWizard: bootstrap.resetWizard,
  };
});

function toLauncherError(error: unknown): LauncherErrorPayload {
  if (
    typeof error === "object" &&
    error !== null &&
    "kind" in error &&
    "message" in error
  ) {
    return error as LauncherErrorPayload;
  }
  return {
    kind: "io",
    message: error instanceof Error ? error.message : String(error),
  };
}
