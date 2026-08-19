<script setup lang="ts">
import { ref } from "vue";
import { Input } from "@/components/ui/input";

const command = ref("");
</script>

<template>
  <section class="settings-plugin-command" aria-label="插件安装命令">
    <Input
      v-model="command"
      class="settings-plugin-command-input font-mono text-sm"
      aria-label="插件安装命令"
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
      placeholder="dsh plugin --profile web add <source>"
    />
  </section>
</template>

<style scoped>
.settings-plugin-command {
  display: grid;
  min-height: 0;
  flex: 1;
  place-items: center;
  padding: 24px;
}

.settings-plugin-command-input {
  width: min(720px, 100%);
}
</style>
