<script setup lang="ts">
// 根组件。对应设计 §M1.5。

import MainView from "@/components/MainView.vue";
import { Toaster } from "@/components/ui/toast";
</script>

<template>
  <MainView />
  <Toaster />
</template>
