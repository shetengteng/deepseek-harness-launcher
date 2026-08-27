import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent } from "vue";
import { beforeEach, expect, test, vi } from "vitest";
import { useInstalledPlugins } from "@/composables/useInstalledPlugins";

const api = vi.hoisted(() => ({ listProfilePlugins: vi.fn() }));

vi.mock("@/lib/tauri", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/tauri")>();
  return { ...actual, ...api };
});

function mountUseInstalledPlugins() {
  let apiHandle: ReturnType<typeof useInstalledPlugins> | null = null;
  const wrapper = mount(
    defineComponent({
      setup() {
        apiHandle = useInstalledPlugins();
        return apiHandle;
      },
      template: "<div />",
    }),
  );
  return { wrapper, api: () => apiHandle! };
}

beforeEach(() => {
  vi.clearAllMocks();
});

test("loads the current plugin list on mount", async () => {
  api.listProfilePlugins.mockResolvedValue({
    profile: "web",
    plugins: [{ name: "dshmarket", spec: "1.31.1" }],
  });
  const { api: installed } = mountUseInstalledPlugins();
  await flushPromises();

  expect(api.listProfilePlugins).toHaveBeenCalledWith("web");
  expect(installed().plugins.value).toEqual([
    { name: "dshmarket", spec: "1.31.1" },
  ]);
  expect(installed().loading.value).toBe(false);
  expect(installed().error.value).toBeNull();
});

test("surfaces a readable error when the list cannot be read", async () => {
  api.listProfilePlugins.mockRejectedValue({
    kind: "dsh_plugin",
    message: "boom",
    user_message: "无法读取已安装插件列表。",
  });
  const { api: installed } = mountUseInstalledPlugins();
  await flushPromises();

  expect(installed().plugins.value).toEqual([]);
  expect(installed().error.value).toBe("无法读取已安装插件列表。");
});
