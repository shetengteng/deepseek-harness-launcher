<script setup lang="ts">
import { useI18n } from "@/lib/i18n";

const { t } = useI18n();
</script>

<template>
  <main class="flex-1 flex items-center justify-center bg-background">
    <section class="flex flex-col items-center gap-[18px] text-center">
      <span
        class="h-8 w-8 rounded-full border-2 border-border border-t-primary animate-spin"
        aria-hidden="true"
      />
      <p class="text-sm font-medium">{{ t("host.starting") }}</p>
    </section>
  </main>
</template>
