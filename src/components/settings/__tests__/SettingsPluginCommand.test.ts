import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, expect, test, vi } from "vitest";

const api = vi.hoisted(() => ({
  runPluginCommand: vi.fn(),
  listProfilePlugins: vi.fn(),
}));

vi.mock("@/lib/tauri", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/tauri")>();
  return { ...actual, ...api };
});

import SettingsPluginCommand from "@/components/settings/SettingsPluginCommand.vue";

function button(wrapper: ReturnType<typeof mount>, label: string) {
  return wrapper.findAll("button").find((item) => item.text().includes(label))!;
}

async function openCommandTab(wrapper: ReturnType<typeof mount>) {
  await button(wrapper, "输入命令").trigger("mousedown", { button: 0 });
  await flushPromises();
}

beforeEach(() => {
  vi.clearAllMocks();
  api.listProfilePlugins.mockResolvedValue({ profile: "web", plugins: [] });
});

test("reviews a validated install command before it runs", async () => {
  const wrapper = mount(SettingsPluginCommand);
  await openCommandTab(wrapper);
  const input = wrapper.get('input[aria-label="插件安装或卸载命令"]');

  expect(input.attributes("placeholder")).toBe(
    "dsh plugin --profile web add <source>",
  );

  await input.setValue("dsh plugin --profile web add github:owner/plugin");
  await wrapper.get("form").trigger("submit");

  expect(wrapper.text()).toContain("确认安装插件");
  expect(wrapper.text()).toContain("github:owner/plugin");
  expect(api.runPluginCommand).not.toHaveBeenCalled();
});

test("shows a format hint instead of running unsupported input", async () => {
  const wrapper = mount(SettingsPluginCommand);
  await openCommandTab(wrapper);
  const input = wrapper.get('input[aria-label="插件安装或卸载命令"]');

  await input.setValue(
    "dsh plugin --profile web add github:owner/plugin && whoami",
  );
  await wrapper.get("form").trigger("submit");

  expect(wrapper.get('[role="alert"]').text()).toContain("add|remove");
  expect(api.runPluginCommand).not.toHaveBeenCalled();
});

test("runs a confirmed remove command and reports success", async () => {
  api.runPluginCommand.mockResolvedValue({
    action: "remove",
    profile: "web",
    source: "github:owner/plugin",
    summary: "removed github:owner/plugin",
  });
  const wrapper = mount(SettingsPluginCommand);
  await openCommandTab(wrapper);
  const input = wrapper.get('input[aria-label="插件安装或卸载命令"]');

  await input.setValue("dsh plugin --profile web remove github:owner/plugin");
  await wrapper.get("form").trigger("submit");
  await button(wrapper, "确认卸载").trigger("click");
  await flushPromises();

  expect(api.runPluginCommand).toHaveBeenCalledWith(
    "dsh plugin --profile web remove github:owner/plugin",
  );
  expect(wrapper.get('[role="status"]').text()).toContain("已完成卸载");
});

test("lists installed plugins and uninstalls from the row button", async () => {
  api.listProfilePlugins.mockResolvedValue({
    profile: "web",
    plugins: [{ name: "dsh-lumina-tarot", spec: "link:/tmp/plugin" }],
  });
  api.runPluginCommand.mockResolvedValue({
    action: "remove",
    profile: "web",
    source: "dsh-lumina-tarot",
    summary: "removed dsh-lumina-tarot",
  });
  const wrapper = mount(SettingsPluginCommand);
  await flushPromises();

  expect(wrapper.text()).toContain("dsh-lumina-tarot");
  await wrapper
    .get('button[aria-label="卸载 dsh-lumina-tarot"]')
    .trigger("click");
  await wrapper
    .get('button[aria-label="确认卸载 dsh-lumina-tarot"]')
    .trigger("click");
  await flushPromises();

  expect(api.runPluginCommand).toHaveBeenCalledWith(
    "dsh plugin --profile web remove dsh-lumina-tarot",
  );
  expect(wrapper.get('[role="status"]').text()).toContain("已完成卸载");
  expect(api.listProfilePlugins).toHaveBeenCalledTimes(2);
});

test("keeps the command form on a separate tab", async () => {
  const wrapper = mount(SettingsPluginCommand);
  await flushPromises();

  expect(wrapper.find('input[aria-label="插件安装或卸载命令"]').exists()).toBe(
    false,
  );
  expect(wrapper.text()).toContain("已安装插件");

  await openCommandTab(wrapper);

  expect(wrapper.find('input[aria-label="插件安装或卸载命令"]').exists()).toBe(
    true,
  );
});
