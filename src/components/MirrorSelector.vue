<script setup lang="ts">
// 镜像源选择器。对应设计 §M2.5 / PR-011。
// 下拉含内置源 + "自定义..." 选项；选中"自定义"才展示 Input，校验通过自动选中。
// 选中内置源会清空自定义源状态，避免双选。

import { computed, ref, watch } from "vue";
import { Loader2, CheckCircle2, AlertCircle } from "lucide-vue-next";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useLauncherStore } from "@/stores/launcher";
import { useI18n } from "@/lib/i18n";

const store = useLauncherStore();
const { t } = useI18n();

/** 下拉里的"自定义"特殊值。选中后展示 Input，但 selectedMirrorId 暂留空。 */
const CUSTOM_VALUE = "__custom__";

/** 本地标记：用户是否处于"自定义模式"（独立于校验状态）。
 *  - 选中"自定义..." → true
 *  - 选中任意内置源 → false
 *  这样即使用户还没输 URL（校验状态 null），输入框也能持续显示。 */
const customMode = ref(false);

/** 当前下拉显示值。 */
const selectValue = computed<string>({
  get: () => {
    if (customMode.value) return CUSTOM_VALUE;
    return store.selectedMirrorId ?? "";
  },
  set: (v: string) => {
    if (v === CUSTOM_VALUE) {
      // 切到自定义模式：清空内置选中，但保留 customMirrorUrl（用户可能正在输入）
      customMode.value = true;
      store.selectMirror("");
    } else {
      // 切回内置源：清空自定义源状态
      customMode.value = false;
      store.customMirrorUrl = "";
      store.customMirrorValidation = null;
      store.selectMirror(v);
    }
  },
});

/** 自定义源是否校验通过（用于显示绿色对勾）。 */
const isCustomSelected = computed(() => {
  const v = store.customMirrorValidation;
  return typeof v === "object" && v !== null;
});

/** 自定义源校验错误信息。 */
const customError = computed(() => {
  const v = store.customMirrorValidation;
  return typeof v === "string" ? v : "";
});

/** 是否显示自定义 URL 输入框。 */
const showCustomInput = computed(() => customMode.value);

// watch 自定义源 URL 输入，debounce 300ms 触发校验
let timer: ReturnType<typeof setTimeout> | null = null;
watch(
  () => store.customMirrorUrl,
  (url) => {
    if (timer) clearTimeout(timer);
    if (!url) {
      store.customMirrorValidation = null;
      return;
    }
    timer = setTimeout(() => {
      void store.validateCustomMirror(url);
    }, 300);
  },
);

// 当自定义源校验通过时，自动选中它（selectMirror(custom.id)）
watch(
  () => store.customMirrorValidation,
  (v) => {
    if (typeof v === "object" && v !== null) {
      store.selectMirror(v.id);
    }
  },
);
</script>

<template>
  <div class="flex flex-col gap-4">
    <!-- 镜像源下拉（内置 + 自定义） -->
    <div class="flex items-center justify-between gap-3">
      <Label for="builtin-mirror">{{ t("mirror.label") }}</Label>
      <button
        v-if="!customMode"
        type="button"
        class="rounded-full px-2 py-1 text-xs text-muted-foreground underline-offset-4 hover:bg-accent hover:text-foreground hover:underline disabled:cursor-not-allowed disabled:opacity-50"
        :disabled="store.wizardStep === 'probing'"
        @click="store.autoPickMirror()"
      >
        {{ store.wizardStep === "probing" ? t("mirror.autoPicking") : t("mirror.autoPick") }}
      </button>
    </div>
    <div class="flex flex-col gap-2">
      <Select v-model="selectValue">
        <SelectTrigger id="builtin-mirror" class="w-full">
          <SelectValue :placeholder="t('mirror.select')" />
        </SelectTrigger>
        <SelectContent class="min-w-[420px]">
          <SelectItem
            v-for="m in store.mirrors.filter((m) => m.trusted)"
            :key="m.id"
            :value="m.id"
          >
            <span>{{ m.name }}</span>
          </SelectItem>
          <SelectItem :value="CUSTOM_VALUE">
            <span class="text-muted-foreground">{{ t("mirror.custom") }}</span>
          </SelectItem>
        </SelectContent>
      </Select>
    </div>

    <!-- 自定义源输入：仅在选中"自定义..."时展示 -->
    <div v-if="showCustomInput" class="flex flex-col gap-2">
      <Label for="custom-mirror">{{ t("mirror.customUrl") }}</Label>
      <div class="relative">
        <Input
          id="custom-mirror"
          v-model="store.customMirrorUrl"
          placeholder="https://your-mirror.com/node"
          :disabled="store.wizardStep === 'probing'"
        />
        <div
          v-if="store.customMirrorUrl"
          class="absolute right-2 top-1/2 -translate-y-1/2"
        >
          <CheckCircle2
            v-if="isCustomSelected"
            class="h-4 w-4 text-green-500"
          />
          <AlertCircle v-else-if="customError" class="h-4 w-4 text-red-500" />
          <Loader2 v-else class="h-4 w-4 animate-spin text-muted-foreground" />
        </div>
      </div>
      <p v-if="customError" class="text-xs text-red-500">
        {{ customError }}
      </p>
      <p v-else-if="isCustomSelected" class="text-xs text-green-600">
        {{ t("mirror.valid") }}
      </p>
    </div>
  </div>
</template>
