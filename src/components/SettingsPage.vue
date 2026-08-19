<script setup lang="ts">
import { ref, watch } from "vue";
import { ArrowLeft, Puzzle, Settings } from "lucide-vue-next";
import SettingsView from "@/components/Settings.vue";
import SettingsMarketplace from "@/components/settings/SettingsMarketplace.vue";
import { useI18n } from "@/lib/i18n";
import {
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";
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
          <nav :aria-label="t('settings.title')">
            <SidebarMenu>
              <SidebarMenuItem class="mb-2">
                <SidebarMenuButton
                  :aria-label="t('settings.backAria')"
                  @click="emit('close')"
                >
                  <ArrowLeft class="h-4 w-4 shrink-0" />
                  {{ t("settings.back") }}
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  :is-active="activeSection === 'settings'"
                  :aria-current="
                    activeSection === 'settings' ? 'page' : undefined
                  "
                  @click="activeSection = 'settings'"
                >
                  <Settings class="h-4 w-4 shrink-0" />
                  {{ t("settings.title") }}
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  :is-active="activeSection === 'plugins'"
                  :aria-current="
                    activeSection === 'plugins' ? 'page' : undefined
                  "
                  @click="activeSection = 'plugins'"
                >
                  <Puzzle class="h-4 w-4 shrink-0" />
                  {{ t("settings.plugins") }}
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </nav>
        </aside>
      </ResizablePanel>

      <ResizableHandle />

      <ResizablePanel :default-size="82" :min-size="70">
        <div class="flex h-full min-w-0 flex-col overflow-hidden">
          <template v-if="activeSection === 'settings'">
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
