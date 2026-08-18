<script setup lang="ts">
import { computed } from "vue";
import { Languages } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import type { ThemeMode } from "@/lib/tauri";
import { useI18n } from "@/lib/i18n";

const props = defineProps<{
  mode: ThemeMode;
  disabled: boolean;
  error: string | null;
}>();

const emit = defineEmits<{ change: [mode: ThemeMode] }>();
const { locale, alternateLocale, setLocale, t } = useI18n();

const blackWhiteEnabled = computed(() => props.mode === "dark");
const description = computed(() =>
  blackWhiteEnabled.value
    ? t("appearance.darkDescription")
    : t("appearance.lightDescription"),
);

function changeTheme(enabled: boolean): void {
  emit("change", enabled ? "dark" : "light");
}
</script>

<template>
  <Card>
    <CardHeader><CardTitle class="text-base">{{ t("appearance.title") }}</CardTitle></CardHeader>
    <CardContent class="space-y-3">
      <div class="flex items-center justify-between gap-4">
        <div class="min-w-0">
          <Label for="black-white-theme">{{ t("appearance.theme") }}</Label>
          <p class="text-xs text-muted-foreground">{{ description }}</p>
        </div>
        <Switch
          id="black-white-theme"
          :model-value="blackWhiteEnabled"
          :disabled="disabled"
          aria-describedby="black-white-theme-description"
          data-testid="black-white-theme-switch"
          @update:model-value="changeTheme"
        />
      </div>
      <p id="black-white-theme-description" class="sr-only">
        {{ description }}
      </p>
      <p v-if="error" class="text-xs text-destructive" role="alert">
        {{ error }}
      </p>
      <div class="flex items-center justify-between gap-4 pt-3">
        <div class="min-w-0">
          <Label>{{ t("language.label") }}</Label>
          <p class="text-xs text-muted-foreground">
            {{ t("language.description") }}
          </p>
        </div>
        <Button
          type="button"
          variant="outline"
          class="rounded-full"
          size="xs"
          :aria-label="t('language.switchTo', { language: t(alternateLocale === 'zh-CN' ? 'language.zh' : 'language.en') })"
          data-testid="language-toggle"
          @click="setLocale(alternateLocale)"
        >
          <Languages class="mr-2 h-4 w-4" aria-hidden="true" />
          {{ t(locale === "zh-CN" ? "language.en" : "language.zh") }}
        </Button>
      </div>
    </CardContent>
  </Card>
</template>
