<script setup lang="ts">
import type { TabsListProps } from "reka-ui";
import type { HTMLAttributes } from "vue";
import { reactiveOmit } from "@vueuse/core";
import { TabsList, useForwardProps } from "reka-ui";
import { cn } from "@/lib/utils";

const props = defineProps<
  TabsListProps & { class?: HTMLAttributes["class"] }
>();
const delegatedProps = reactiveOmit(props, "class");
const forwarded = useForwardProps(delegatedProps);
</script>

<template>
  <TabsList
    v-bind="forwarded"
    :class="
      cn(
        'inline-flex items-center rounded-md bg-muted p-[3px] text-muted-foreground',
        props.class,
      )
    "
  >
    <slot />
  </TabsList>
</template>
