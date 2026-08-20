<script setup lang="ts">
import { computed } from "vue";
import { Languages } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { useI18n } from "@/lib/i18n";

const { alternateLocale, setLocale, t } = useI18n();

const actionLabel = computed(() =>
  t("language.switchTo", {
    language: t(
      alternateLocale.value === "zh-CN" ? "language.zh" : "language.en",
    ),
  }),
);

function toggleLocale(): void {
  setLocale(alternateLocale.value);
}
</script>

<template>
  <Button
    type="button"
    variant="ghost"
    size="xs"
    class="size-7 px-0"
    :aria-label="actionLabel"
    :title="actionLabel"
    data-testid="language-toggle"
    @click="toggleLocale"
  >
    <Languages aria-hidden="true" />
  </Button>
</template>
