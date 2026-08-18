import { defineStore } from "pinia";
import { shallowRef } from "vue";
import { getTheme, setTheme, type ThemeMode } from "@/lib/tauri";

const THEME_CACHE_KEY = "deepseek-harness-launcher.theme";

function isThemeMode(value: string | null): value is ThemeMode {
  return value === "light" || value === "dark";
}

function cachedTheme(): ThemeMode {
  if (typeof window === "undefined") return "light";
  try {
    const value = window.localStorage.getItem(THEME_CACHE_KEY);
    return isThemeMode(value) ? value : "light";
  } catch {
    return "light";
  }
}

function cacheTheme(theme: ThemeMode): void {
  try {
    window.localStorage.setItem(THEME_CACHE_KEY, theme);
  } catch {
    // localStorage 仅用于启动时避免闪烁，失败不影响权威状态的保存。
  }
}

function applyTheme(theme: ThemeMode): void {
  document.documentElement.classList.toggle("dark", theme === "dark");
}

function messageOf(error: unknown): string {
  if (typeof error === "object" && error !== null) {
    if ("user_message" in error) {
      return String((error as { user_message: unknown }).user_message);
    }
    if ("message" in error) {
      return String((error as { message: unknown }).message);
    }
  }
  return String(error);
}

export const useThemeStore = defineStore("theme", () => {
  const mode = shallowRef<ThemeMode>(cachedTheme());
  const initializing = shallowRef(false);
  const saving = shallowRef(false);
  const error = shallowRef<string | null>(null);
  let initialization: Promise<void> | null = null;

  function apply(modeToApply: ThemeMode): void {
    mode.value = modeToApply;
    applyTheme(modeToApply);
    cacheTheme(modeToApply);
  }

  function initialize(): Promise<void> {
    if (initialization) return initialization;
    initializing.value = true;
    initialization = (async () => {
      try {
        apply(await getTheme());
      } catch (initializationError) {
        apply(mode.value);
        error.value = messageOf(initializationError);
      } finally {
        initializing.value = false;
      }
    })();
    return initialization;
  }

  async function updateTheme(nextMode: ThemeMode): Promise<boolean> {
    await initialize();
    if (saving.value || nextMode === mode.value) return true;

    const previousMode = mode.value;
    saving.value = true;
    error.value = null;
    apply(nextMode);
    try {
      await setTheme(nextMode);
      return true;
    } catch (saveError) {
      apply(previousMode);
      error.value = messageOf(saveError);
      return false;
    } finally {
      saving.value = false;
    }
  }

  return { mode, initializing, saving, error, initialize, updateTheme };
});
