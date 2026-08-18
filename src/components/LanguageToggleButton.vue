<script setup lang="ts">
import { Languages } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { useI18n } from "@/lib/i18n";

const { alternateLocale, setLocale, t } = useI18n();

function toggleLocale(): void {
  setLocale(alternateLocale.value);
}
</script>

<template>
  <Button
    type="button"
    variant="ghost"
    size="xs"
    :aria-label="t('language.switchTo', { language: t(alternateLocale === 'zh-CN' ? 'language.zh' : 'language.en') })"
    data-testid="language-toggle"
    @click="toggleLocale"
  >
    <Languages class="mr-1 h-4 w-4" aria-hidden="true" />
    {{ t(alternateLocale === "zh-CN" ? "language.zh" : "language.en") }}
  </Button>
</template>
