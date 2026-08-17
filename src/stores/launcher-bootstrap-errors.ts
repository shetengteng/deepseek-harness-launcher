import type { LauncherErrorPayload } from "@/lib/tauri";

export function isNodeInstallCancelled(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "kind" in error &&
    (error as { kind: unknown }).kind === "node_install_cancelled"
  );
}

export function isDshInstallCancelled(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "kind" in error &&
    "message" in error &&
    (error as { kind: unknown }).kind === "dsh_install" &&
    String((error as { message: unknown }).message).includes("cancelled")
  );
}

export function errorMessage(error: unknown): string {
  return typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string"
    ? (error as { message: string }).message
    : error instanceof Error
      ? error.message
      : String(error);
}

export function toLauncherError(error: unknown): LauncherErrorPayload {
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
