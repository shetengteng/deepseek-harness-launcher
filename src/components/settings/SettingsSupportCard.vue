<script setup lang="ts">
import { FileArchive, RefreshCw } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useI18n } from "@/lib/i18n";

defineProps<{
  exporting: boolean;
  exportInfo: string | null;
  confirmingUninstall: boolean;
  uninstalling: boolean;
  uninstallError: string | null;
}>();

defineEmits<{
  export: [];
  confirmUninstall: [];
  cancelUninstall: [];
  uninstall: [];
}>();
const { t } = useI18n();
</script>

<template>
  <Card>
    <CardHeader><CardTitle class="text-base">{{ t("support.title") }}</CardTitle></CardHeader>
    <CardContent class="space-y-3"
      ><div class="flex items-center justify-between gap-4">
        <div class="min-w-0">
          <div class="text-sm font-medium">{{ t("support.exportTitle") }}</div>
          <p class="text-xs text-muted-foreground">
            {{ t("support.exportDescription") }}
          </p>
        </div>
        <Button
          variant="outline"
          size="xs"
          :disabled="exporting"
          @click="$emit('export')"
          ><FileArchive class="mr-2 h-4 w-4" />{{
            exporting ? t("support.exporting") : t("support.export")
          }}</Button
        >
      </div>
      <div v-if="exportInfo" class="text-xs break-all text-muted-foreground">
        {{ exportInfo }}
      </div></CardContent
    >
  </Card>

  <Card>
    <CardHeader><CardTitle class="text-base">{{ t("uninstall.title") }}</CardTitle></CardHeader>
    <CardContent class="space-y-3"
      ><div class="flex items-center justify-between gap-4">
        <div class="min-w-0">
          <div class="text-sm font-medium">{{ t("uninstall.runtime") }}</div>
          <p class="text-xs text-muted-foreground">
            {{ t("uninstall.description") }}
          </p>
        </div>
        <Button
          v-if="!confirmingUninstall"
          variant="destructive"
          size="xs"
          @click="$emit('confirmUninstall')"
          ><RefreshCw class="mr-2 h-4 w-4" />{{ t("uninstall.action") }}</Button
        >
      </div>
      <div
        v-if="confirmingUninstall"
        class="space-y-3 rounded border border-destructive/40 bg-destructive/5 p-3"
      >
        <p class="text-sm">
          {{ t("uninstall.confirm") }}
        </p>
        <div class="flex justify-end gap-2">
          <Button
            variant="outline"
            size="xs"
            :disabled="uninstalling"
            @click="$emit('cancelUninstall')"
            >{{ t("common.cancel") }}</Button
          ><Button
            variant="destructive"
            size="xs"
            :disabled="uninstalling"
            @click="$emit('uninstall')"
            ><RefreshCw class="mr-2 h-4 w-4" />{{
            uninstalling ? t("uninstall.running") : t("uninstall.exit")
            }}</Button
          >
        </div>
        <p v-if="uninstallError" class="text-sm text-destructive">
          {{ uninstallError }}
        </p>
      </div></CardContent
    >
  </Card>
</template>
