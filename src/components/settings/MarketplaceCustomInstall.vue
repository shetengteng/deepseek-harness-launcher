<script setup lang="ts">
import { ChevronDown } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface Props {
  customError: string | null;
}

defineProps<Props>();
const emit = defineEmits<{
  submitCustom: [];
}>();

const customExpanded = defineModel<boolean>("customExpanded", {
  required: true,
});
const customCommand = defineModel<string>("customCommand", { required: true });
</script>

<template>
  <section
    class="marketplace-custom-install"
    aria-labelledby="custom-install-title"
  >
    <Button
      variant="ghost"
      size="xs"
      class="marketplace-custom-trigger"
      :aria-expanded="customExpanded"
      aria-controls="custom-install-content"
      @click="customExpanded = !customExpanded"
    >
      <span class="marketplace-custom-prefix" aria-hidden="true">+</span>
      <span id="custom-install-title">自定义安装</span>
      <ChevronDown
        :class="[
          'h-3 w-3 transition-transform duration-150',
          customExpanded && 'rotate-180',
        ]"
        aria-hidden="true"
      />
    </Button>
    <form
      v-if="customExpanded"
      id="custom-install-content"
      class="marketplace-custom-form"
      @submit.prevent="emit('submitCustom')"
    >
      <Input
        v-model="customCommand"
        class="font-mono text-xs"
        placeholder="dsh plugin --profile web add <source>"
        aria-label="自定义插件安装命令"
        :aria-invalid="Boolean(customError)"
        :aria-describedby="customError ? 'custom-command-error' : undefined"
      />
      <Button type="submit" size="sm">继续</Button>
    </form>
    <p
      v-if="customError"
      id="custom-command-error"
      class="marketplace-custom-error"
      role="alert"
    >
      {{ customError }}
    </p>
  </section>
</template>

<style scoped>
.marketplace-custom-install {
  margin-top: 10px;
}
.marketplace-custom-trigger {
  padding-inline: 2px;
  color: hsl(var(--muted-foreground));
}
.marketplace-custom-prefix {
  color: hsl(var(--muted-foreground));
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 13px;
}
.marketplace-custom-form {
  display: flex;
  gap: 8px;
  margin-top: 7px;
}
.marketplace-custom-error {
  margin: 6px 0 0;
  color: hsl(var(--destructive));
  font-size: 11px;
}
@media (max-width: 620px) {
  .marketplace-custom-form {
    flex-direction: column;
  }
}
</style>
