<script setup lang="ts">
import { ref, watch } from "vue";
import { ArrowLeft, Puzzle, Settings } from "lucide-vue-next";
import SettingsView from "@/components/Settings.vue";
import SettingsMarketplace from "@/components/settings/SettingsMarketplace.vue";
import { useI18n } from "@/lib/i18n";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";

type SettingsSection = "settings" | "plugins";

const emit = defineEmits<{
  close: [];
  upgradeReady: [origin: string];
  nodeUpdated: [version: string];
}>();
const props = withDefaults(
  defineProps<{
    nodeVersion?: string | null;
    hostOrigin?: string | null;
    exportDiagnosticsRequest?: number;
    section?: SettingsSection;
  }>(),
  {
    nodeVersion: null,
    hostOrigin: null,
    exportDiagnosticsRequest: 0,
    section: "settings",
  },
);

const activeSection = ref<SettingsSection>(props.section);
const { t } = useI18n();

watch(
  () => props.section,
  (section) => {
    activeSection.value = section;
  },
);
</script>

<template>
  <section class="h-screen overflow-hidden bg-background">
    <ResizablePanelGroup direction="horizontal" class="h-full">
      <ResizablePanel :default-size="18" :min-size="14" :max-size="30">
        <aside
          class="h-full overflow-hidden bg-muted/30 p-3"
          :aria-label="t('settings.title')"
        >
          <button
            type="button"
            class="flex h-9 w-full items-center gap-2 rounded-md px-2 text-left text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
            :aria-label="t('settings.backAria')"
            @click="emit('close')"
          >
            <ArrowLeft class="h-4 w-4 shrink-0" />
            {{ t("settings.back") }}
          </button>
          <nav class="space-y-1">
            <button
              type="button"
              class="flex h-9 w-full items-center gap-2 rounded-md px-2 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground"
              :class="
                activeSection === 'settings'
                  ? 'bg-background font-medium text-foreground shadow-sm ring-1 ring-border'
                  : 'text-muted-foreground'
              "
              :aria-current="activeSection === 'settings' ? 'page' : undefined"
              @click="activeSection = 'settings'"
            >
              <Settings class="h-4 w-4 shrink-0" />
              {{ t("settings.title") }}
            </button>
            <button
              type="button"
              class="flex h-9 w-full items-center gap-2 rounded-md px-2 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground"
              :class="
                activeSection === 'plugins'
                  ? 'bg-background font-medium text-foreground shadow-sm ring-1 ring-border'
                  : 'text-muted-foreground'
              "
              :aria-current="activeSection === 'plugins' ? 'page' : undefined"
              @click="activeSection = 'plugins'"
            >
              <Puzzle class="h-4 w-4 shrink-0" />
              {{ t("settings.plugins") }}
            </button>
          </nav>
        </aside>
      </ResizablePanel>

      <ResizableHandle />

      <ResizablePanel :default-size="82" :min-size="70">
        <div class="flex h-full min-w-0 flex-col overflow-hidden">
          <template v-if="activeSection === 'settings'">
            <div class="shrink-0 border-b px-7 py-5">
              <h1 class="text-lg font-semibold">{{ t("settings.title") }}</h1>
              <p class="mt-1 text-sm text-muted-foreground">
                {{ t("settings.description") }}
              </p>
            </div>
            <SettingsView
              class="min-h-0 flex-1"
              :node-version="props.nodeVersion"
              :host-origin="props.hostOrigin"
              :export-diagnostics-request="props.exportDiagnosticsRequest"
              @upgrade-ready="emit('upgradeReady', $event)"
              @node-updated="emit('nodeUpdated', $event)"
            />
          </template>

          <SettingsMarketplace v-else />
        </div>
      </ResizablePanel>
    </ResizablePanelGroup>
  </section>
</template>
