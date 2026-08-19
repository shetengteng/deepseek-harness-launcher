<script setup lang="ts">
import type { HTMLAttributes } from "vue";
import { cn } from "@/lib/utils";

const props = defineProps<{
  class?: HTMLAttributes["class"];
}>();
</script>

<template>
  <li :class="cn('group/sidebar-menu-item relative', props.class)">
    <slot />
  </li>
</template>
