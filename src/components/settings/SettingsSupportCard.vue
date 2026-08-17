<script setup lang="ts">
import { FileArchive } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

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
</script>

<template>
  <Card>
    <CardHeader><CardTitle class="text-base">问题排查</CardTitle></CardHeader>
    <CardContent
      ><div class="flex items-center justify-between gap-4 py-3">
        <div class="min-w-0">
          <div class="text-sm font-medium">导出排查资料</div>
          <p class="text-xs text-muted-foreground">
            打包应用状态和日志，便于反馈问题
          </p>
        </div>
        <Button
          variant="outline"
          size="xs"
          :disabled="exporting"
          @click="$emit('export')"
          ><FileArchive class="mr-2 h-4 w-4" />{{
            exporting ? "导出中…" : "导出"
          }}</Button
        >
      </div>
      <div v-if="exportInfo" class="text-xs break-all text-muted-foreground">
        {{ exportInfo }}
      </div></CardContent
    >
  </Card>

  <Card>
    <CardHeader><CardTitle class="text-base">卸载</CardTitle></CardHeader>
    <CardContent class="space-y-3"
      ><div class="flex items-center justify-between gap-4 py-3">
        <div class="min-w-0">
          <div class="text-sm font-medium">移除 DeepSeek Harness</div>
          <p class="text-xs text-muted-foreground">
            删除托管的 DeepSeek Harness、Node.js
            运行时和设置，保留启动器与诊断日志
          </p>
        </div>
        <Button
          v-if="!confirmingUninstall"
          variant="destructive"
          size="xs"
          @click="$emit('confirmUninstall')"
          >卸载</Button
        >
      </div>
      <div
        v-if="confirmingUninstall"
        class="space-y-3 rounded border border-destructive/40 bg-destructive/5 p-3"
      >
        <p class="text-sm">
          确认后将立即关闭应用。重新打开启动器后，需要重新安装 DeepSeek
          Harness。
        </p>
        <div class="flex justify-end gap-2">
          <Button
            variant="outline"
            size="xs"
            :disabled="uninstalling"
            @click="$emit('cancelUninstall')"
            >取消</Button
          ><Button
            variant="destructive"
            size="xs"
            :disabled="uninstalling"
            @click="$emit('uninstall')"
            >{{ uninstalling ? "正在卸载…" : "卸载并退出" }}</Button
          >
        </div>
        <p v-if="uninstallError" class="text-sm text-destructive">
          {{ uninstallError }}
        </p>
      </div></CardContent
    >
  </Card>
</template>
