import {
  listMirrors,
  probeMirrors,
  setRegistry,
  validateCustomMirror,
} from "@/lib/tauri";
import { errorMessage, toLauncherError } from "./launcher-bootstrap-errors";
import type { LauncherState } from "./launcher-state";
import type { LastAction } from "./launcher-types";

type FailureHandler = (error: unknown, action?: LastAction) => void;

export function createMirrorActions(
  state: LauncherState,
  fail: FailureHandler,
) {
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

  async function setRegistryAction(registry: string): Promise<boolean> {
    try {
      await setRegistry(registry);
      if (state.bootstrapPlan.value) {
        state.bootstrapPlan.value.registry = registry;
      }
      return true;
    } catch (error) {
      fail(error);
      return false;
    }
  }

  return {
    loadMirrors,
    autoPickMirror,
    validateCustomMirrorAction,
    selectMirror,
    setRegistryAction,
  };
}
