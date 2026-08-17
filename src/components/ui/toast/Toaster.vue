<script setup lang="ts">
import { isVNode } from "vue"
import { Toast, ToastClose, ToastDescription, ToastProvider, ToastTitle, ToastViewport } from "."
import { useToast } from "./use-toast"

const { toasts } = useToast()
</script>

<template>
  <ToastProvider>
    <Toast v-for="toast in toasts" :key="toast.id" v-bind="toast">
      <div class="grid w-full gap-1">
        <ToastTitle v-if="toast.title">
          {{ toast.title }}
        </ToastTitle>
        <template v-if="toast.description">
          <ToastDescription v-if="isVNode(toast.description)">
            <component :is="toast.description" />
          </ToastDescription>
          <ToastDescription v-else>
            {{ toast.description }}
          </ToastDescription>
        </template>
        <div v-if="toast.action" class="mt-2 flex justify-end">
          <component :is="toast.action" />
        </div>
        <ToastClose />
      </div>
    </Toast>
    <ToastViewport />
  </ToastProvider>
</template>
