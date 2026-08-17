import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, expect, test, vi } from "vitest";

const api = vi.hoisted(() =>
  Object.fromEntries(
    [
      "getDshState",
      "getLatestDshVersion",
      "installDsh",
      "setNodeMirror",
      "setRegistry",
      "listMirrors",
      "exportDiagnostics",
      "uninstallManagedRuntime",
    ].map((name) => [name, vi.fn()]),
  ),
);

vi.mock("@/lib/tauri", () => api);

import Settings from "@/components/Settings.vue";

function button(wrapper: ReturnType<typeof mount>, label: string) {
  return wrapper.findAll("button").find((item) => item.text().trim() === label)!;
}

function state(current: string | null) {
  return {
    current,
    known_good: null,
    pending: null,
    installed: [],
    node_mirror: "https://nodejs.org/dist",
    registry: "https://registry.npmjs.org",
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  api.getDshState.mockResolvedValue(state("0.0.9"));
  api.listMirrors.mockResolvedValue([]);
  api.getLatestDshVersion.mockResolvedValue({ latest_version: "0.1.0" });
});

test("requires explicit confirmation before uninstalling managed runtime", async () => {
  api.uninstallManagedRuntime.mockResolvedValue(undefined);
  const wrapper = mount(Settings);
  await flushPromises();

  await button(wrapper, "卸载").trigger("click");
  expect(api.uninstallManagedRuntime).not.toHaveBeenCalled();

  await button(wrapper, "卸载并退出").trigger("click");
  expect(api.uninstallManagedRuntime).toHaveBeenCalledTimes(1);
});

test("updates only to the registry latest version after explicit confirmation", async () => {
  api.installDsh.mockResolvedValue("0.1.0");
  const wrapper = mount(Settings);
  await flushPromises();

  expect(wrapper.text()).toContain("0.1.0");
  await button(wrapper, "更新到最新版本").trigger("click");
  await flushPromises();

  expect(api.installDsh).toHaveBeenCalledWith(true);
  expect(wrapper.emitted("upgradeReady")?.[0]).toEqual(["0.1.0"]);
});

test("disables the update when current version is already latest", async () => {
  api.getDshState.mockResolvedValue(state("0.1.0"));
  const wrapper = mount(Settings);
  await flushPromises();

  expect(button(wrapper, "已是最新版本").attributes("disabled")).toBeDefined();
  expect(api.installDsh).not.toHaveBeenCalled();
});

test("retains the displayed current version when installing latest fails", async () => {
  api.installDsh.mockRejectedValue({ message: "网络中断，请检查 npm 下载源后重试。" });
  const wrapper = mount(Settings);
  await flushPromises();

  await button(wrapper, "更新到最新版本").trigger("click");
  await flushPromises();

  expect(wrapper.text()).toContain("0.0.9");
  expect(wrapper.text()).toContain("网络中断，请检查 npm 下载源后重试。");
  expect(wrapper.emitted("upgradeReady")).toBeUndefined();
});