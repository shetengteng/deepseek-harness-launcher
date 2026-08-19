import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, expect, test, vi } from "vitest";

const api = vi.hoisted(() => ({ runPluginCommand: vi.fn() }));

vi.mock("@/lib/tauri", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/tauri")>();
  return { ...actual, ...api };
});

import SettingsPluginCommand from "@/components/settings/SettingsPluginCommand.vue";

function button(wrapper: ReturnType<typeof mount>, label: string) {
  return wrapper.findAll("button").find((item) => item.text().includes(label))!;
}

beforeEach(() => {
  vi.clearAllMocks();
});

test("reviews a validated install command before it runs", async () => {
  const wrapper = mount(SettingsPluginCommand);
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
