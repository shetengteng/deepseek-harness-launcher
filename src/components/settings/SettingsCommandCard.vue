<script setup lang="ts">
import { TerminalSquare } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DshCliInstallResult } from "@/lib/tauri";
import { useI18n } from "@/lib/i18n";

defineProps<{
  installing: boolean;
  result: DshCliInstallResult | null;
  error: string | null;
}>();

defineEmits<{ install: [] }>();
const { t } = useI18n();
</script>

<template>
  <Card>
    <CardHeader><CardTitle class="text-base">{{ t("command.title") }}</CardTitle></CardHeader>
    <CardContent class="space-y-3">
      <div class="flex items-center justify-between gap-4">
        <div class="min-w-0">
          <div class="text-sm font-medium">{{ t("command.installTitle") }}</div>
          <p class="text-xs text-muted-foreground">
            {{ t("command.description") }}
          </p>
        </div>
        <Button
          variant="outline"
          size="xs"
          class="rounded-full"
          :disabled="installing"
          @click="$emit('install')"
        >
          <TerminalSquare class="mr-2 h-4 w-4" />{{
            installing ? t("command.installing") : t("command.install")
          }}
        </Button>
      </div>
      <div v-if="result" class="space-y-1 text-xs text-muted-foreground">
        <p>{{ t("command.installed") }}<code class="break-all">{{ result.command_path }}</code></p>
        <p>{{ result.path_instruction }}</p>
      </div>
      <p v-if="error" class="text-sm text-destructive">{{ error }}</p>
    </CardContent>
  </Card>
</template>
