export type LauncherPhase =
  "booting" | "first_run" | "idle" | "ready" | "error";

export type WizardStep =
  | "mirror_select"
  | "probing"
  | "resolving"
  | "downloading"
  | "extracting"
  | "done"
  | "failed";

export type LastAction =
  | "bootstrap"
  | "installNode"
  | "installDsh"
  | "startHost"
  | "shutdownHost"
  | null;

export const DEFAULT_NODE_VERSION = "24.18.1";
