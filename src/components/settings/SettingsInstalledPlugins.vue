<script setup lang="ts">
import { LoaderCircle } from "lucide-vue-next";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { useI18n } from "@/lib/i18n";
import type { InstalledPlugin } from "@/lib/tauri";
import SettingsInstalledPluginItem from "./SettingsInstalledPluginItem.vue";

defineProps<{
  plugins: readonly InstalledPlugin[];
  loading: boolean;
  error: string | null;
  busyName: string | null;
  disabled: boolean;
}>();

const emit = defineEmits<{
  remove: [name: string];
}>();

const { t } = useI18n();
</script>

<template>
  <Card
    class="settings-installed-plugins flex min-h-0 flex-1 flex-col overflow-hidden"
  >
    <CardHeader class="shrink-0 gap-2 pb-4">
      <p class="text-xs text-muted-foreground">
        {{ t("pluginList.description") }}
      </p>
    </CardHeader>
    <CardContent
      class="settings-installed-plugins-body flex min-h-0 flex-1 flex-col overflow-hidden"
    >
      <p
        v-if="loading"
        class="flex items-center gap-2 text-sm text-muted-foreground"
      >
        <LoaderCircle class="h-4 w-4 animate-spin" />
        {{ t("pluginList.loading") }}
      </p>
      <p v-else-if="error" class="text-sm text-destructive" role="alert">
        {{ error }}
      </p>
      <p v-else-if="plugins.length === 0" class="text-sm text-muted-foreground">
        {{ t("pluginList.empty") }}
      </p>
      <ul v-else class="settings-installed-plugins-list overflow-y-auto">
        <li v-for="plugin in plugins" :key="plugin.name">
          <SettingsInstalledPluginItem
            :plugin="plugin"
            :busy="busyName === plugin.name"
            :disabled="disabled"
            @remove="emit('remove', $event)"
          />
        </li>
      </ul>
    </CardContent>
  </Card>
</template>

<style scoped>
.settings-installed-plugins {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  overflow: hidden;
  box-shadow: 0 14px 30px -24px hsl(var(--foreground) / 0.42);
}

.settings-installed-plugins-body {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  overflow: hidden;
}

.settings-installed-plugins-list {
  min-height: 0;
  flex: 1;
  margin: 0;
  padding: 0 2px 0 0;
  list-style: none;
  overscroll-behavior: contain;
}

.settings-installed-plugins-list li + li {
  border-top: 1px solid hsl(var(--border));
}
</style>
