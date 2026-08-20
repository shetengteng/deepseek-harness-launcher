import { mount } from "@vue/test-utils";
import { afterEach, expect, test } from "vitest";
import LanguageToggleButton from "@/components/LanguageToggleButton.vue";
import { setLocale } from "@/lib/i18n";

afterEach(() => {
  setLocale("zh-CN");
});

test("keeps the first-run language toggle as an icon-only control", async () => {
  const wrapper = mount(LanguageToggleButton);
  const button = wrapper.get('[data-testid="language-toggle"]');

  expect(button.text()).toBe("");
  expect(button.attributes("aria-label")).toBe("切换为 English");
  await button.trigger("click");

  expect(button.attributes("aria-label")).toBe("Switch to 中文");
});
