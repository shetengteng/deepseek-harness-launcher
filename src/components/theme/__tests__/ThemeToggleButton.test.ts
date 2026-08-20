import { mount } from "@vue/test-utils";
import { expect, test } from "vitest";
import ThemeToggleButton from "@/components/theme/ThemeToggleButton.vue";

test("offers the opposite theme from the first-run page", async () => {
  const wrapper = mount(ThemeToggleButton, {
    props: { mode: "dark", disabled: false, saving: false, error: null },
  });

  const button = wrapper.get('[data-testid="first-run-theme-toggle"]');
  expect(button.text()).toBe("");
  expect(button.attributes("aria-label")).toBe("切换为浅色主题");
  await button.trigger("click");

  expect(wrapper.emitted("change")).toEqual([["light"]]);
});
