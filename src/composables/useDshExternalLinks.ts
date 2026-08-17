import { onMounted, onUnmounted, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useLauncherStore } from "@/stores/launcher";

export function useDshExternalLinks() {
  const store = useLauncherStore();
  const dshFrame = ref<HTMLIFrameElement | null>(null);

  function currentDshOrigin(): string | null {
    if (!store.origin) return null;
    try {
      return new URL(store.origin).origin;
    } catch {
      return null;
    }
  }

  function handleDshExternalLink(event: MessageEvent<unknown>): void {
    const dshOrigin = currentDshOrigin();
    const frame = dshFrame.value;
    if (
      !dshOrigin ||
      !frame ||
      event.source !== frame.contentWindow ||
      event.origin !== dshOrigin ||
      typeof event.data !== "object" ||
      event.data === null ||
      !("type" in event.data) ||
      !("href" in event.data) ||
      event.data.type !== "dsh:open-external" ||
      typeof event.data.href !== "string"
    ) {
      return;
    }

    let target: URL;
    try {
      target = new URL(event.data.href);
    } catch {
      return;
    }
    if (
      (target.protocol !== "http:" && target.protocol !== "https:") ||
      target.origin === dshOrigin
    ) {
      return;
    }

    void openUrl(target.href).catch((error: unknown) => {
      console.warn("failed to open dsh external link:", error);
    });
  }

  onMounted(() => {
    window.addEventListener("message", handleDshExternalLink);
  });

  onUnmounted(() => {
    window.removeEventListener("message", handleDshExternalLink);
  });

  return { dshFrame };
}
