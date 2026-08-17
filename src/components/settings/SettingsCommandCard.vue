<script setup lang="ts">
import { TerminalSquare } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DshCliInstallResult } from "@/lib/tauri";

defineProps<{
  installing: boolean;
  result: DshCliInstallResult | null;
  error: string | null;
}>();

defineEmits<{ install: [] }>();
</script>

<template>
  <Card>
    <CardHeader><CardTitle class="text-base">命令行</CardTitle></CardHeader>
    <CardContent class="space-y-3">
      <div class="flex items-center justify-between gap-4">
        <div class="min-w-0">
          <div class="text-sm font-medium">安装 <code>dsh</code> 命令</div>
          <p class="text-xs text-muted-foreground">
            在新终端中管理与启动器相同的 DeepSeek Harness profile
          </p>
        </div>
        <Button
          variant="outline"
          size="xs"
          :disabled="installing"
          @click="$emit('install')"
        >
          <TerminalSquare class="mr-2 h-4 w-4" />{{
            installing ? "安装中…" : "安装命令"
          }}
        </Button>
      </div>
      <div v-if="result" class="space-y-1 text-xs text-muted-foreground">
        <p>已安装：<code class="break-all">{{ result.command_path }}</code></p>
        <p>{{ result.path_instruction }}</p>
      </div>
      <p v-if="error" class="text-sm text-destructive">{{ error }}</p>
    </CardContent>
  </Card>
</template>
