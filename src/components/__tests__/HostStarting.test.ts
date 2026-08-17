import { mount } from "@vue/test-utils";
import { expect, test } from "vitest";

import HostStarting from "@/components/HostStarting.vue";

test("shows the product name without the launch command", () => {
  const wrapper = mount(HostStarting);

  expect(wrapper.text()).toBe("正在启动 DeepSeek Harness…");
  expect(wrapper.text()).not.toContain("node");
});
