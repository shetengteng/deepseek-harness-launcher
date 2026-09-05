<script setup lang="ts">
import { useI18n } from "@/lib/i18n";

defineProps<{
  hint?: string | null;
  technical?: string | null;
}>();

const { t } = useI18n();
</script>

<template>
  <div
    v-if="hint || technical"
    class="rounded-md border border-destructive/30 bg-destructive/5 p-3 text-sm"
    role="alert"
    data-testid="dsh-start-error"
  >
    <div class="mb-1 text-xs font-medium text-destructive">
      {{ t("update.startError") }}
    </div>
    <p
      v-if="hint"
      class="break-words text-xs leading-5 text-foreground/80"
    >
      {{ hint }}
    </p>
    <pre
      v-if="technical"
      class="mt-2 max-h-[160px] overflow-auto whitespace-pre-wrap break-words rounded border bg-muted/30 p-2 font-mono text-[11px] text-foreground/80"
    >{{ technical }}</pre>
  </div>
</template>
