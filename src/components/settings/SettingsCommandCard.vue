<script setup lang="ts">
import { TerminalSquare, Trash2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DshCliStatus } from "@/lib/tauri";
import { useI18n } from "@/lib/i18n";

const props = defineProps<{
  status: DshCliStatus | null;
  installing: boolean;
  uninstalling: boolean;
  error: string | null;
}>();

defineEmits<{ install: []; uninstall: [] }>();
const { t } = useI18n();
</script>

<template>
  <Card>
    <CardHeader
      ><CardTitle class="text-base">{{
        t("command.title")
      }}</CardTitle></CardHeader
    >
    <CardContent class="space-y-3">
      <div class="flex items-center justify-between gap-4">
        <div class="min-w-0">
          <div class="text-sm font-medium">
            {{
              props.status?.state === "installed"
                ? t("command.installedTitle")
                : props.status?.state === "conflict"
                  ? t("command.conflictTitle")
                  : t("command.installTitle")
            }}
          </div>
          <p class="text-xs text-muted-foreground">
            {{
              props.status?.state === "installed"
                ? t("command.installedDescription")
                : props.status?.state === "conflict"
                  ? t("command.conflictDescription")
                  : t("command.description")
            }}
          </p>
        </div>
        <Button
          v-if="props.status?.state === 'installed'"
          variant="outline"
          size="xs"
          class="rounded-full"
          :disabled="uninstalling"
          @click="$emit('uninstall')"
        >
          <Trash2 class="mr-2 h-4 w-4" />{{
            uninstalling ? t("command.uninstalling") : t("command.uninstall")
          }}
        </Button>
        <Button
          v-else-if="props.status?.state !== 'conflict'"
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
      <div
        v-if="props.status?.state === 'installed'"
        class="space-y-1 text-xs text-muted-foreground"
      >
        <p>
          {{ t("command.installed")
          }}<code class="break-all">{{ props.status.command_path }}</code>
        </p>
        <p>{{ props.status.path_instruction }}</p>
      </div>
      <p v-if="error" class="text-sm text-destructive">{{ error }}</p>
    </CardContent>
  </Card>
</template>
