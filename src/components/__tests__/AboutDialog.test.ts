import { flushPromises, shallowMount } from "@vue/test-utils";
import { beforeEach, expect, test, vi } from "vitest";

const api = vi.hoisted(() => ({ getAboutInfo: vi.fn() }));

vi.mock("@/lib/tauri", () => api);

import AboutDialog from "@/components/AboutDialog.vue";

const stubs = {
  Dialog: { template: "<div><slot /></div>" },
  DialogContent: { template: "<section><slot /></section>" },
  DialogTitle: { template: "<h2><slot /></h2>" },
};

beforeEach(() => {
  vi.clearAllMocks();
  api.getAboutInfo.mockResolvedValue({
    launcher_version: "0.1.0",
    dsh_version: "0.1.0-rc.6",
    node_version: "22.19.0",
    data_directory: "/Users/example/Library/Application Support/io.deepseek.DeepSeek/deepseek-harness-launcher",
  });
});

test("shows the managed runtime details from the launcher", async () => {
  const wrapper = shallowMount(AboutDialog, {
    props: { open: true, hostOrigin: "http://127.0.0.1:51842/" },
    global: { stubs },
  });
  await flushPromises();

  expect(api.getAboutInfo).toHaveBeenCalledOnce();
  expect(wrapper.text()).toContain("deepseek-harness-launcher");
  expect(wrapper.text()).toContain("DeepSeek Harness 版本");
  expect(wrapper.text()).toContain("0.1.0-rc.6");
  expect(wrapper.text()).toContain("Node.js 版本");
  expect(wrapper.text()).toContain("127.0.0.1:51842");
  expect(wrapper.text()).toContain("数据目录");
});

test("only loads runtime details when the dialog opens", async () => {
  const wrapper = shallowMount(AboutDialog, {
    props: { open: false },
    global: { stubs },
  });
  await flushPromises();
  expect(api.getAboutInfo).not.toHaveBeenCalled();

  await wrapper.setProps({ open: true });
  await flushPromises();
  expect(api.getAboutInfo).toHaveBeenCalledOnce();
});
