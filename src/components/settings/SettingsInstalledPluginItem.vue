<script setup lang="ts">
import { shallowRef, watch } from "vue";
import { LoaderCircle, Trash2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { useI18n } from "@/lib/i18n";
import type { InstalledPlugin } from "@/lib/tauri";

const props = defineProps<{
  plugin: InstalledPlugin;
  busy: boolean;
  disabled: boolean;
}>();

const emit = defineEmits<{
  remove: [name: string];
}>();

const pending = shallowRef(false);
const { t } = useI18n();

watch(
  () => props.busy,
  (busy) => {
    if (!busy) pending.value = false;
  },
);

function requestRemove(): void {
  if (props.disabled) return;
  pending.value = true;
}

function cancelRemove(): void {
  pending.value = false;
}

function confirmRemove(): void {
  if (props.disabled) return;
  emit("remove", props.plugin.name);
}
</script>

<template>
  <div class="settings-installed-plugin-item">
    <div class="min-w-0">
      <p class="truncate text-sm font-medium">{{ plugin.name }}</p>
      <p class="truncate font-mono text-xs text-muted-foreground">
        {{ plugin.spec }}
      </p>
    </div>
    <div class="flex shrink-0 items-center gap-2">
      <template v-if="pending">
        <Button
          type="button"
          variant="outline"
          size="xs"
          :disabled="disabled"
          @click="cancelRemove"
        >
          {{ t("pluginList.cancel") }}
        </Button>
        <Button
          type="button"
          variant="destructive"
          size="xs"
          :disabled="disabled"
          :aria-label="`${t('pluginList.confirm')} ${plugin.name}`"
          @click="confirmRemove"
        >
          <LoaderCircle v-if="busy" class="animate-spin" />
          <Trash2 v-else />
          {{ busy ? t("pluginCommand.running") : t("pluginList.confirm") }}
        </Button>
      </template>
      <Button
        v-else
        type="button"
        variant="outline"
        size="xs"
        :disabled="disabled"
        :aria-label="`${t('pluginList.uninstall')} ${plugin.name}`"
        @click="requestRemove"
      >
        <Trash2 />{{ t("pluginList.uninstall") }}
      </Button>
    </div>
  </div>
</template>

<style scoped>
.settings-installed-plugin-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 0;
}
</style>
