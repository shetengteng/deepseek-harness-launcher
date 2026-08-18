<script setup lang="ts">
import { computed } from "vue";
import { Moon, Sun } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import type { ThemeMode } from "@/lib/tauri";
import { useI18n } from "@/lib/i18n";

const props = defineProps<{
  mode: ThemeMode;
  disabled: boolean;
  saving: boolean;
  error: string | null;
}>();

const emit = defineEmits<{ change: [mode: ThemeMode] }>();
const { t } = useI18n();

const nextMode = computed<ThemeMode>(() =>
  props.mode === "dark" ? "light" : "dark",
);
const actionLabel = computed(() =>
  nextMode.value === "dark" ? t("theme.switchToDark") : t("theme.switchToLight"),
);

function toggleTheme(): void {
  if (!props.disabled) emit("change", nextMode.value);
}
</script>

<template>
  <div class="flex flex-col items-end gap-1">
    <Button
      type="button"
      variant="ghost"
      size="xs"
      :disabled="disabled"
      :aria-label="actionLabel"
      data-testid="first-run-theme-toggle"
      @click="toggleTheme"
    >
      <Sun v-if="mode === 'dark'" aria-hidden="true" />
      <Moon v-else aria-hidden="true" />
      {{ saving ? t("theme.saving") : actionLabel }}
    </Button>
    <p
      v-if="error"
      class="max-w-44 text-right text-xs text-destructive"
      role="alert"
    >
      {{ error }}
    </p>
  </div>
</template>
