import { mount } from "@vue/test-utils";
import { beforeEach, expect, test } from "vitest";
import SettingsAppearanceCard from "@/components/settings/SettingsAppearanceCard.vue";
import { setLocale } from "@/lib/i18n";

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
  storage.clear();
  Object.defineProperty(window, "localStorage", {
    configurable: true,
    value: localStorageMock,
  });
  setLocale("zh-CN");
});

test("switches from black-white to light theme", async () => {
  const wrapper = mount(SettingsAppearanceCard, {
    props: { mode: "dark", disabled: false, error: null },
  });

  expect(wrapper.text()).toContain("黑色背景，白色文字");
  await wrapper
    .get('[data-testid="black-white-theme-switch"]')
    .trigger("click");

  expect(wrapper.emitted("change")).toEqual([["light"]]);
});

test("switches the appearance card to English and persists the selection", async () => {
  const wrapper = mount(SettingsAppearanceCard, {
    props: { mode: "light", disabled: false, error: null },
  });

  await wrapper.get('[data-testid="language-toggle"]').trigger("click");

  expect(document.documentElement.lang).toBe("en-US");
  expect(storage.get("deepseek-harness-launcher.locale")).toBe("en-US");
  expect(wrapper.text()).toContain("Appearance");
  expect(wrapper.text()).toContain("Language");
});
