<script setup lang="ts">
import { ref, watch } from "vue";
import { ArrowLeft, Puzzle, Settings } from "lucide-vue-next";
import SettingsView from "@/components/Settings.vue";

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

watch(
  () => props.section,
  (section) => {
    activeSection.value = section;
  },
);
</script>

<template>
  <section class="flex min-h-screen flex-1 flex-col bg-background">
    <header class="flex h-14 shrink-0 items-center border-b px-5">
      <div class="min-w-0">
        <p class="truncate text-sm font-medium">deepseek-harness-launcher</p>
        <p class="text-xs text-muted-foreground">设置</p>
      </div>
    </header>

    <div class="flex min-h-0 flex-1">
      <aside
        class="w-52 shrink-0 border-r bg-muted/30 p-3"
        aria-label="设置导航"
      >
        <button
          type="button"
          class="flex h-9 w-full items-center gap-2 rounded-md px-2 text-left text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
          aria-label="返回 DeepSeek Harness"
          @click="emit('close')"
        >
          <ArrowLeft class="h-4 w-4 shrink-0" />
          返回
        </button>
        <div class="my-3 border-t" />
        <p class="px-2 pb-2 pt-1 text-xs font-medium text-muted-foreground">
          设置
        </p>
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
            设置
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
            插件
          </button>
        </nav>
      </aside>

      <div class="flex min-w-0 flex-1 flex-col">
        <template v-if="activeSection === 'settings'">
          <div class="shrink-0 border-b px-7 py-5">
            <h1 class="text-lg font-semibold">设置</h1>
            <p class="mt-1 text-sm text-muted-foreground">
              管理 DeepSeek Harness 的运行时、更新源和诊断信息。
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

        <section
          v-else
          class="flex min-h-0 flex-1 items-center justify-center p-8"
          aria-labelledby="plugins-page-title"
        >
          <div class="max-w-sm text-center">
            <div
              class="mx-auto flex h-12 w-12 items-center justify-center rounded-lg bg-muted"
            >
              <Puzzle
                class="h-6 w-6 text-muted-foreground"
                aria-hidden="true"
              />
            </div>
            <h1 id="plugins-page-title" class="mt-4 text-lg font-semibold">
              插件
            </h1>
            <p class="mt-2 text-sm leading-6 text-muted-foreground">
              插件市场和已安装插件管理将在后续版本提供。
            </p>
          </div>
        </section>
      </div>
    </div>
  </section>
</template>
