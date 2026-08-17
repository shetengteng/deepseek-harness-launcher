import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, expect, test, vi } from "vitest";

const api = vi.hoisted(() =>
  Object.fromEntries(
    [
      "getDshState",
      "getLatestDshVersion",
      "installDsh",
      "restartHostAfterDshUpdate",
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
  return wrapper
    .findAll("button")
    .find((item) => item.text().trim() === label)!;
}

function state(current: string | null) {
  return {
    current,
    known_good: null,
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
  api.restartHostAfterDshUpdate.mockResolvedValue({
    origin: "http://127.0.0.1:1337/",
    active_version: "0.1.0",
    rolled_back: false,
  });
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
  await button(wrapper, "安装新版本").trigger("click");
  await flushPromises();

  expect(api.installDsh).toHaveBeenCalledWith({ expectedVersion: "0.1.0" });
  expect(api.restartHostAfterDshUpdate).toHaveBeenCalledOnce();
  expect(wrapper.emitted("upgradeReady")?.[0]).toEqual([
    "http://127.0.0.1:1337/",
  ]);
});

test("keeps the update status height fixed when the current version is latest", async () => {
  api.getDshState.mockResolvedValue(state("0.1.0"));
  const wrapper = mount(Settings);
  await flushPromises();

  expect(wrapper.text()).not.toContain("可更新版本");
  expect(wrapper.text()).not.toContain("安装新版本");
  expect(wrapper.text()).toContain("正在使用的 DeepSeek Harness 版本");
  expect(wrapper.text()).not.toContain("当前启动的 DeepSeek Harness");
  expect(wrapper.text()).toContain("已是最新版本");
  expect(wrapper.find('[data-testid="dsh-update-status"]').classes()).toContain(
    "text-xs",
  );
  expect(api.installDsh).not.toHaveBeenCalled();

  await button(wrapper, "刷新").trigger("click");
  await flushPromises();

  expect(api.getLatestDshVersion).toHaveBeenCalledTimes(2);
});

test("shows the current DeepSeek Harness IP address and port", async () => {
  const wrapper = mount(Settings, {
    props: { hostOrigin: "http://127.0.0.1:51842/" },
  });
  await flushPromises();

  expect(wrapper.text()).toContain("运行 IP 与端口");
  expect(wrapper.text()).toContain("127.0.0.1:51842");
});

test("does not expose the automatic recovery version in settings", async () => {
  api.getDshState.mockResolvedValue({
    ...state("0.0.9"),
    known_good: "0.0.8",
  });
  const wrapper = mount(Settings);
  await flushPromises();

  expect(wrapper.text()).not.toContain("可恢复的版本");
  expect(wrapper.text()).not.toContain("0.0.8");
});

test("retains the displayed current version when installing latest fails", async () => {
  api.installDsh.mockRejectedValue({
    message: "网络中断，请检查 npm 下载源后重试。",
  });
  const wrapper = mount(Settings);
  await flushPromises();

  await button(wrapper, "安装新版本").trigger("click");
  await flushPromises();

  expect(wrapper.text()).toContain("0.0.9");
  expect(wrapper.text()).toContain("网络中断，请检查 npm 下载源后重试。");
  expect(wrapper.emitted("upgradeReady")).toBeUndefined();
});
