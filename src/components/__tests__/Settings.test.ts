import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, expect, test, vi } from "vitest";

const api = vi.hoisted(() =>
  Object.fromEntries(
    [
      "getDshState",
      "getLatestDshVersion",
      "getNodeUpdateTarget",
      "installDshCli",
      "installDsh",
      "upgradeNode",
      "restartHostAfterDshUpdate",
      "setNodeMirror",
      "setRegistry",
      "listMirrors",
      "exportDiagnostics",
      "uninstallManagedRuntime",
    ].map((name) => [name, vi.fn()]),
  ),
);
const eventListeners = vi.hoisted(
  () =>
    new Map<
      string,
      (event: {
        payload: { stage: string; bytes: number; total: number | null };
      }) => void
    >(),
);

vi.mock("@/lib/tauri", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/tauri")>();
  return { ...actual, ...api };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (
      event: string,
      handler: (event: {
        payload: { stage: string; bytes: number; total: number | null };
      }) => void,
    ) => {
      eventListeners.set(event, handler);
      return Promise.resolve(vi.fn());
    },
  ),
}));

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
  eventListeners.clear();
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

test("updates only Node after the manual confirmation", async () => {
  api.getNodeUpdateTarget.mockResolvedValue({
    current_version: "22.19.0",
    target_version: "24.4.0",
    engines_node: ">=22.0.0",
    target_source: "dsh-engines",
    update_available: true,
  });
  api.upgradeNode.mockResolvedValue("24.4.0");
  const wrapper = mount(Settings, {
    attachTo: document.body,
    props: { nodeVersion: "22.19.0" },
  });
  await flushPromises();

  await button(wrapper, "更新 Node").trigger("click");
  await flushPromises();

  expect(api.getNodeUpdateTarget).toHaveBeenCalledOnce();
  expect(document.body.textContent).toContain("更新 Node.js");
  expect(document.body.textContent).toContain("24.4.0");
  expect(api.upgradeNode).not.toHaveBeenCalled();

  [...document.querySelectorAll("button")]
    .find((item) => item.textContent?.trim() === "仅更新 Node")
    ?.click();
  await flushPromises();

  expect(api.upgradeNode).toHaveBeenCalledWith({
    version: "24.4.0",
    operationId: expect.any(String),
  });
  expect(api.installDsh).not.toHaveBeenCalled();
  expect(api.restartHostAfterDshUpdate).not.toHaveBeenCalled();
  expect(wrapper.emitted("nodeUpdated")?.[0]).toEqual(["24.4.0"]);
  wrapper.unmount();
});

test("shows download progress while manually updating Node", async () => {
  api.getNodeUpdateTarget.mockResolvedValue({
    current_version: "22.19.0",
    target_version: "24.4.0",
    engines_node: ">=22.0.0",
    target_source: "dsh-engines",
    update_available: true,
  });
  let finishUpgrade: ((version: string) => void) | undefined;
  api.upgradeNode.mockImplementation(
    () => new Promise<string>((resolve) => (finishUpgrade = resolve)),
  );
  const wrapper = mount(Settings, {
    attachTo: document.body,
    props: { nodeVersion: "22.19.0" },
  });
  await flushPromises();

  await button(wrapper, "更新 Node").trigger("click");
  await flushPromises();
  [...document.querySelectorAll("button")]
    .find((item) => item.textContent?.trim() === "仅更新 Node")
    ?.click();
  await flushPromises();

  eventListeners.get("download-progress")?.({
    payload: { stage: "download", bytes: 50, total: 100 },
  });
  await flushPromises();

  expect(document.body.textContent).toContain("正在下载并校验 Node.js 运行时");
  expect(document.body.textContent).toContain("45%");

  finishUpgrade?.("24.4.0");
  await flushPromises();
  wrapper.unmount();
});

test("allows a verified fallback Node runtime to be reinstalled", async () => {
  api.getNodeUpdateTarget.mockResolvedValue({
    current_version: "22.19.0",
    target_version: "24.18.1",
    engines_node: null,
    target_source: "launcher-verified-fallback",
    update_available: true,
  });
  api.upgradeNode.mockResolvedValue("24.18.1");
  const wrapper = mount(Settings, {
    attachTo: document.body,
    props: { nodeVersion: "22.19.0" },
  });
  await flushPromises();

  await button(wrapper, "更新 Node").trigger("click");
  await flushPromises();

  expect(document.body.textContent).toContain("launcher 已验证的版本");
  expect(document.body.textContent).toContain("仅更新 Node");

  [...document.querySelectorAll("button")]
    .find((item) => item.textContent?.trim() === "仅更新 Node")
    ?.click();
  await flushPromises();

  expect(api.upgradeNode).toHaveBeenCalledWith({
    version: "24.18.1",
    operationId: expect.any(String),
  });
  expect(api.installDsh).not.toHaveBeenCalled();
  wrapper.unmount();
});

test("installs the dsh command and shows the PATH instructions", async () => {
  api.installDshCli.mockResolvedValue({
    command_path: "/Users/test/.local/bin/dsh",
    path_instruction: "关闭并重新打开 Terminal。",
  });
  const wrapper = mount(Settings);
  await flushPromises();

  await button(wrapper, "安装命令").trigger("click");
  await flushPromises();

  expect(api.installDshCli).toHaveBeenCalledOnce();
  expect(wrapper.text()).toContain("/Users/test/.local/bin/dsh");
  expect(wrapper.text()).toContain("关闭并重新打开 Terminal。");
});

test("shows an actionable error when the dsh command cannot be installed", async () => {
  api.installDshCli.mockRejectedValue({ message: "目标已有其他 dsh 命令" });
  const wrapper = mount(Settings);
  await flushPromises();

  await button(wrapper, "安装命令").trigger("click");
  await flushPromises();

  expect(wrapper.text()).toContain("目标已有其他 dsh 命令");
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

test("asks to confirm a Node upgrade before changing the current runtime", async () => {
  api.installDsh.mockRejectedValueOnce({
    kind: "node_upgrade_required",
    message: "dsh 0.2.0 requires Node >=24.0.0",
    data: {
      dsh_version: "0.1.0",
      current_node: "22.19.0",
      engines_node: ">=24.0.0",
      suggested_node: "24.4.0",
    },
  });
  const wrapper = mount(Settings, { attachTo: document.body });
  await flushPromises();

  await button(wrapper, "安装新版本").trigger("click");
  await flushPromises();

  expect(api.upgradeNode).not.toHaveBeenCalled();
  expect(document.body.textContent).toContain("需要升级 Node");
  expect(document.body.textContent).toContain("24.4.0");
  expect(wrapper.text()).toContain("0.0.9");

  [...document.querySelectorAll("button")]
    .find((item) => item.textContent?.trim() === "取消更新")
    ?.click();
  await flushPromises();

  expect(api.upgradeNode).not.toHaveBeenCalled();
  expect(api.installDsh).toHaveBeenCalledTimes(1);
  expect(document.body.textContent).not.toContain("需要升级 Node");
  expect(wrapper.text()).toContain("0.0.9");
  wrapper.unmount();
});

test("upgrades Node after confirmation and then installs the displayed dsh", async () => {
  api.installDsh
    .mockRejectedValueOnce({
      kind: "node_upgrade_required",
      message: "dsh 0.1.0 requires Node >=24.0.0",
      data: {
        dsh_version: "0.1.0",
        current_node: "22.19.0",
        engines_node: ">=24.0.0",
        suggested_node: "24.4.0",
      },
    })
    .mockResolvedValueOnce("0.1.0");
  api.upgradeNode.mockResolvedValue("24.4.0");
  const wrapper = mount(Settings, { attachTo: document.body });
  await flushPromises();

  await button(wrapper, "安装新版本").trigger("click");
  await flushPromises();
  [...document.querySelectorAll("button")]
    .find((item) => item.textContent?.trim() === "确认升级并继续")
    ?.click();
  await flushPromises();

  expect(api.upgradeNode).toHaveBeenCalledWith({
    version: "24.4.0",
    operationId: expect.any(String),
  });
  expect(api.installDsh).toHaveBeenNthCalledWith(2, {
    expectedVersion: "0.1.0",
  });
  expect(api.restartHostAfterDshUpdate).toHaveBeenCalledOnce();
  expect(wrapper.emitted("upgradeReady")?.[0]).toEqual([
    "http://127.0.0.1:1337/",
  ]);
  wrapper.unmount();
});
