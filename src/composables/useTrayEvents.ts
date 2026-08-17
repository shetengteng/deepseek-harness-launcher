import { onBeforeUnmount, onMounted } from "vue";

interface TrayEventHandlers {
  openSettings: () => void;
  checkDshUpdate: () => void;
  exportDiagnostics: () => void;
  openAbout: () => void;
  hostRestarted: () => void;
}

export function useTrayEvents(handlers: TrayEventHandlers): void {
  const unlisteners: Array<() => void> = [];

  onMounted(() => {
    void register();
  });

  onBeforeUnmount(() => {
    unlisteners.splice(0).forEach((unlisten) => unlisten());
  });

  async function register(): Promise<void> {
    const { listen } = await import("@tauri-apps/api/event");
    const listeners = await Promise.all([
      listen("tray-open-settings", handlers.openSettings),
      listen("tray-check-dsh-update", handlers.checkDshUpdate),
      listen("tray-export-diagnostics", handlers.exportDiagnostics),
      listen("tray-open-about", handlers.openAbout),
      listen("tray-host-restarted", handlers.hostRestarted),
    ]);
    unlisteners.push(...listeners);
  }
}
