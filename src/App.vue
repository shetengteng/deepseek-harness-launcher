<script setup lang="ts">
// 根组件。对应设计 §M1.5。
// M1 简化：仅挂载 MainView，不引入 Toast 容器（M2 接入首启向导时再加）。

import MainView from "@/components/MainView.vue";
</script>

<template>
  <MainView />
</template>
