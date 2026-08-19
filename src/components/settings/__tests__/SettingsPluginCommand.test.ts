import { mount } from "@vue/test-utils";
import { expect, test } from "vitest";
import SettingsPluginCommand from "@/components/settings/SettingsPluginCommand.vue";

test("keeps only an editable plugin install command input", async () => {
  const wrapper = mount(SettingsPluginCommand);
  const input = wrapper.get('input[aria-label="插件安装命令"]');

  expect(wrapper.findAll("input")).toHaveLength(1);
  expect(input.attributes("placeholder")).toBe(
    "dsh plugin --profile web add <source>",
  );

  await input.setValue("dsh plugin --profile web add github:owner/plugin");

  expect((input.element as HTMLInputElement).value).toBe(
    "dsh plugin --profile web add github:owner/plugin",
  );
});
