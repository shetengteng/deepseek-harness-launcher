import {
  restartHost,
  rollbackDsh,
  shutdownHost,
  startHost,
  type CrashLimitPayload,
  type HostRestartedPayload,
} from "@/lib/tauri";
import type { LauncherState } from "./launcher-state";
import type { LastAction } from "./launcher-types";

type FailureHandler = (error: unknown, action?: LastAction) => void;

export function createHostActions(
  state: LauncherState,
  fail: FailureHandler,
  refreshStatus: () => Promise<void>,
) {
  function setHostReady(origin: string): void {
    state.hostSession.value += 1;
    state.origin.value = origin;
    state.phase.value = "ready";
    state.error.value = null;
    state.lastFailedAction.value = null;
    state.starting.value = false;
  }

  function markHostRestarting(): void {
    state.starting.value = true;
  }

  function failHostRestart(message: string): void {
    state.starting.value = false;
    fail({ kind: "host", message }, "startHost");
  }

  async function startHostAction(): Promise<void> {
    if (state.starting.value) return;
    state.starting.value = true;
    try {
      setHostReady(await startHost());
    } catch (error) {
      fail(error, "startHost");
    } finally {
      state.starting.value = false;
    }
  }

  async function shutdownHostAction(): Promise<void> {
    if (state.stopping.value) return;
    state.stopping.value = true;
    try {
      await shutdownHost();
      state.origin.value = null;
      if (state.phase.value === "ready") state.phase.value = "idle";
    } catch (error) {
      fail(error, "shutdownHost");
    } finally {
      state.stopping.value = false;
    }
  }

  async function retryAfterCrash(): Promise<void> {
    if (state.crashRecovering.value) return;
    state.crashRecovering.value = true;
    try {
      setHostReady(await restartHost());
      state.crashLimit.value = null;
    } catch (error) {
      fail(error, "startHost");
    } finally {
      state.crashRecovering.value = false;
    }
  }

  async function rollbackAfterCrash(): Promise<void> {
    if (state.crashRecovering.value) return;
    state.crashRecovering.value = true;
    try {
      await rollbackDsh();
      setHostReady(await restartHost());
      state.crashLimit.value = null;
      await refreshStatus();
    } catch (error) {
      fail(error, "startHost");
    } finally {
      state.crashRecovering.value = false;
    }
  }

  function dismissCrash(): void {
    state.crashLimit.value = null;
    if (state.phase.value === "ready") {
      state.phase.value = "idle";
      state.origin.value = null;
    }
  }

  async function initCrashEvents(): Promise<void> {
    const { listen } = await import("@tauri-apps/api/event");
    await listen<CrashLimitPayload>("host-crash-limit", (event) => {
      state.crashLimit.value = event.payload;
    });
    await listen<HostRestartedPayload>("host-restarted", (event) => {
      setHostReady(event.payload.origin);
      state.crashLimit.value = null;
      state.autoRestartedAttempt.value = event.payload.attempt;
      setTimeout(() => {
        state.autoRestartedAttempt.value = null;
      }, 5000);
    });
  }

  return {
    setHostReady,
    markHostRestarting,
    failHostRestart,
    startHostAction,
    shutdownHostAction,
    retryAfterCrash,
    rollbackAfterCrash,
    dismissCrash,
    initCrashEvents,
  };
}
