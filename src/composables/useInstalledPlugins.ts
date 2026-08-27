import { onMounted, readonly, shallowRef } from "vue";
import {
  listProfilePlugins,
  type InstalledPlugin,
  type LauncherErrorPayload,
} from "@/lib/tauri";
import { useI18n } from "@/lib/i18n";

const DEFAULT_PROFILE = "web";

export function useInstalledPlugins(options: { profile?: string } = {}) {
  const profile = options.profile ?? DEFAULT_PROFILE;
  const { t } = useI18n();
  const plugins = shallowRef<InstalledPlugin[]>([]);
  const loading = shallowRef(true);
  const error = shallowRef<string | null>(null);

  async function reload(): Promise<void> {
    loading.value = true;
    error.value = null;
    try {
      const snapshot = await listProfilePlugins(profile);
      plugins.value = snapshot.plugins;
    } catch (caught) {
      const payload = caught as LauncherErrorPayload;
      error.value =
        payload.user_message ?? payload.message ?? t("pluginList.failed");
    } finally {
      loading.value = false;
    }
  }

  onMounted(() => {
    void reload();
  });

  return {
    plugins: readonly(plugins),
    loading: readonly(loading),
    error: readonly(error),
    reload,
  };
}
