<script setup lang="ts">
import type { HTMLAttributes } from "vue";
import { cn } from "@/lib/utils";

const props = withDefaults(
  defineProps<{
    class?: HTMLAttributes["class"];
    isActive?: boolean;
  }>(),
  {
    isActive: false,
  },
);
</script>

<template>
  <button
    v-bind="$attrs"
    type="button"
    :data-active="props.isActive ? 'true' : undefined"
    :class="
      cn(
        'flex min-h-9 w-full items-center gap-2 rounded-md px-2 text-left text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
        props.isActive &&
          'bg-accent font-medium text-accent-foreground',
        props.class,
      )
    "
  >
    <slot />
  </button>
</template>
