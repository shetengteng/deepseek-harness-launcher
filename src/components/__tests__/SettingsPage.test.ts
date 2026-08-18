import { mount } from "@vue/test-utils";
import { expect, test } from "vitest";
import SettingsPage from "@/components/SettingsPage.vue";

function button(wrapper: ReturnType<typeof mount>, label: string) {
  return wrapper.findAll("button").find((item) => item.text().includes(label))!;
}

test("switches between settings and the plugin placeholder", async () => {
  const wrapper = mount(SettingsPage, {
    global: {
      stubs: {
        SettingsView: { template: "<div>设置内容</div>" },
      },
    },
  });

  expect(wrapper.text()).toContain("设置内容");
  expect(wrapper.get('[aria-current="page"]').text()).toContain("设置");

  await button(wrapper, "插件").trigger("click");

  expect(wrapper.text()).toContain(
    "插件市场和已安装插件管理将在后续版本提供。",
  );
  expect(wrapper.get('[aria-current="page"]').text()).toContain("插件");
});

test("shows the requested section when it changes", async () => {
  const wrapper = mount(SettingsPage, {
    props: { section: "settings" },
    global: {
      stubs: {
        SettingsView: { template: "<div>设置内容</div>" },
      },
    },
  });

  await wrapper.setProps({ section: "plugins" });

  expect(wrapper.text()).toContain(
    "插件市场和已安装插件管理将在后续版本提供。",
  );
});

test("returns to the launcher when requested", async () => {
  const wrapper = mount(SettingsPage, {
    global: {
      stubs: {
        SettingsView: { template: "<div />" },
      },
    },
  });

  await button(wrapper, "返回").trigger("click");

  expect(wrapper.emitted("close")).toHaveLength(1);
});
