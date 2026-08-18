import { mount } from "@vue/test-utils";
import { expect, test } from "vitest";
import SettingsPage from "@/components/SettingsPage.vue";
import { ResizablePanelGroup } from "@/components/ui/resizable";

function button(wrapper: ReturnType<typeof mount>, label: string) {
  return wrapper.findAll("button").find((item) => item.text().includes(label))!;
}

test("switches between settings and the plugin marketplace", async () => {
  const wrapper = mount(SettingsPage, {
    global: {
      stubs: {
        SettingsView: { template: "<div>设置内容</div>" },
        SettingsMarketplace: { template: "<div>插件市场内容</div>" },
      },
    },
  });

  expect(wrapper.text()).toContain("设置内容");
  expect(wrapper.findComponent(ResizablePanelGroup).exists()).toBe(true);
  expect(wrapper.get("section").classes()).toContain("overflow-hidden");
  expect(wrapper.get("aside").classes()).toContain("overflow-hidden");
  expect(wrapper.get("aside").find(".border-t").exists()).toBe(false);
  expect(wrapper.get('[aria-current="page"]').text()).toContain("设置");

  await button(wrapper, "插件").trigger("click");

  expect(wrapper.text()).toContain("插件市场内容");
  expect(wrapper.get('[aria-current="page"]').text()).toContain("插件");
});

test("shows the requested section when it changes", async () => {
  const wrapper = mount(SettingsPage, {
    props: { section: "settings" },
    global: {
      stubs: {
        SettingsView: { template: "<div>设置内容</div>" },
        SettingsMarketplace: { template: "<div>插件市场内容</div>" },
      },
    },
  });

  await wrapper.setProps({ section: "plugins" });

  expect(wrapper.text()).toContain("插件市场内容");
});

test("returns to the launcher when requested", async () => {
  const wrapper = mount(SettingsPage, {
    global: {
      stubs: {
        SettingsView: { template: "<div />" },
        SettingsMarketplace: { template: "<div />" },
      },
    },
  });

  await button(wrapper, "返回").trigger("click");

  expect(wrapper.emitted("close")).toHaveLength(1);
});
