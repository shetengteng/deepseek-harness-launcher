import { mount } from "@vue/test-utils";
import { expect, test } from "vitest";
import SettingsPage from "@/components/SettingsPage.vue";
import { ResizablePanelGroup } from "@/components/ui/resizable";
import {
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";

function button(wrapper: ReturnType<typeof mount>, label: string) {
  return wrapper.findAll("button").find((item) => item.text().includes(label))!;
}

test("switches between settings and the plugin command input", async () => {
  const wrapper = mount(SettingsPage, {
    global: {
      stubs: {
        SettingsView: { template: "<div>设置内容</div>" },
        SettingsPluginCommand: {
          template: '<div><input aria-label="插件安装命令" /></div>',
        },
      },
    },
  });

  expect(wrapper.text()).toContain("设置内容");
  expect(wrapper.findComponent(ResizablePanelGroup).exists()).toBe(true);
  expect(wrapper.get("section").classes()).toContain("overflow-hidden");
  expect(wrapper.get("aside").classes()).toContain("overflow-hidden");
  expect(wrapper.get("aside").find(".border-t").exists()).toBe(false);
  expect(wrapper.findAllComponents(SidebarMenuItem)).toHaveLength(3);
  expect(wrapper.findAllComponents(SidebarMenuButton)).toHaveLength(3);
  expect(wrapper.find('[data-active="true"]').text()).toContain("设置");
  expect(wrapper.get('[aria-current="page"]').text()).toContain("设置");

  await button(wrapper, "插件").trigger("click");

  expect(wrapper.find('input[aria-label="插件安装命令"]').exists()).toBe(true);
  expect(wrapper.get('[aria-current="page"]').text()).toContain("插件");
});

test("shows the requested section when it changes", async () => {
  const wrapper = mount(SettingsPage, {
    props: { section: "settings" },
    global: {
      stubs: {
        SettingsView: { template: "<div>设置内容</div>" },
        SettingsPluginCommand: {
          template: '<div><input aria-label="插件安装命令" /></div>',
        },
      },
    },
  });

  await wrapper.setProps({ section: "plugins" });

  expect(wrapper.find('input[aria-label="插件安装命令"]').exists()).toBe(true);
});

test("returns to the launcher when requested", async () => {
  const wrapper = mount(SettingsPage, {
    global: {
      stubs: {
        SettingsView: { template: "<div />" },
        SettingsPluginCommand: { template: "<div />" },
      },
    },
  });

  await button(wrapper, "返回").trigger("click");

  expect(wrapper.emitted("close")).toHaveLength(1);
});
