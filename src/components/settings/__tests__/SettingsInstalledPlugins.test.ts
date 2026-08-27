import { mount } from "@vue/test-utils";
import { expect, test } from "vitest";
import SettingsInstalledPlugins from "@/components/settings/SettingsInstalledPlugins.vue";

test("renders each installed plugin with an uninstall button", () => {
  const wrapper = mount(SettingsInstalledPlugins, {
    props: {
      plugins: [
        { name: "dsh-lumina-tarot", spec: "link:/tmp/plugin" },
        { name: "dshmarket", spec: "1.31.1" },
      ],
      loading: false,
      error: null,
      busyName: null,
      disabled: false,
    },
  });

  expect(wrapper.text()).toContain("dsh-lumina-tarot");
  expect(wrapper.text()).toContain("dshmarket");
  expect(
    wrapper.findAll("button").filter((item) => item.text().includes("卸载")),
  ).toHaveLength(2);
});

test("asks for confirmation before emitting remove", async () => {
  const wrapper = mount(SettingsInstalledPlugins, {
    props: {
      plugins: [{ name: "dshmarket", spec: "1.31.1" }],
      loading: false,
      error: null,
      busyName: null,
      disabled: false,
    },
  });

  await wrapper.get('button[aria-label="卸载 dshmarket"]').trigger("click");
  expect(wrapper.emitted("remove")).toBeUndefined();

  await wrapper.get('button[aria-label="确认卸载 dshmarket"]').trigger("click");
  expect(wrapper.emitted("remove")).toEqual([["dshmarket"]]);
});

test("shows an empty state when no third-party plugins are installed", () => {
  const wrapper = mount(SettingsInstalledPlugins, {
    props: {
      plugins: [],
      loading: false,
      error: null,
      busyName: null,
      disabled: false,
    },
  });

  expect(wrapper.text()).toContain("还没有已安装的第三方插件");
});

test("scrolls the plugin list when it overflows", () => {
  const wrapper = mount(SettingsInstalledPlugins, {
    props: {
      plugins: [
        { name: "dsh-alpha", spec: "1.0.0" },
        { name: "dsh-beta", spec: "1.0.0" },
      ],
      loading: false,
      error: null,
      busyName: null,
      disabled: false,
    },
  });

  expect(wrapper.get("ul").classes()).toContain("overflow-y-auto");
});
