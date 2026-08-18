import { createPinia, setActivePinia } from "pinia";
import { beforeEach, expect, test, vi } from "vitest";

const api = vi.hoisted(() => ({
  getTheme: vi.fn(),
  setTheme: vi.fn(),
}));

vi.mock("@/lib/tauri", () => api);

import { useThemeStore } from "@/stores/theme";

const storage = new Map<string, string>();
const localStorageMock: Storage = {
  get length() {
    return storage.size;
  },
  clear: () => storage.clear(),
  getItem: (key) => storage.get(key) ?? null,
  key: (index) => [...storage.keys()][index] ?? null,
  removeItem: (key) => {
    storage.delete(key);
  },
  setItem: (key, value) => {
    storage.set(key, value);
  },
};

beforeEach(() => {
  vi.clearAllMocks();
  storage.clear();
  Object.defineProperty(window, "localStorage", {
    configurable: true,
    value: localStorageMock,
  });
  document.documentElement.className = "";
  setActivePinia(createPinia());
});

test("starts with the light theme when no cached preference exists", () => {
  const theme = useThemeStore();

  expect(theme.mode).toBe("light");
});

test("uses the persisted theme to update the launcher root element", async () => {
  api.getTheme.mockResolvedValue("light");
  const theme = useThemeStore();

  await theme.initialize();

  expect(theme.mode).toBe("light");
  expect(document.documentElement.classList.contains("dark")).toBe(false);
  expect(storage.get("deepseek-harness-launcher.theme")).toBe("light");
});

test("updates immediately and rolls back when the theme cannot be saved", async () => {
  api.getTheme.mockResolvedValue("dark");
  let rejectSave: ((reason?: unknown) => void) | undefined;
  api.setTheme.mockImplementation(
    () =>
      new Promise<void>((_resolve, reject) => {
        rejectSave = reject;
      }),
  );
  const theme = useThemeStore();
  await theme.initialize();

  const saving = theme.updateTheme("light");
  await Promise.resolve();

  expect(theme.mode).toBe("light");
  expect(document.documentElement.classList.contains("dark")).toBe(false);
  expect(api.setTheme).toHaveBeenCalledWith("light");

  rejectSave?.({ user_message: "无法保存主题，请重试。" });

  await expect(saving).resolves.toBe(false);
  expect(theme.mode).toBe("dark");
  expect(document.documentElement.classList.contains("dark")).toBe(true);
  expect(theme.error).toBe("无法保存主题，请重试。");
});
